use anyhow::Context as _;
use clap::{Parser, Subcommand};
use common_utils::Deferred;
use rusb::UsbContext;
use std::fs;
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::{
    fs::{OpenOptionsExt, PermissionsExt, chown},
    net::{UnixListener, UnixStream},
};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[cfg(test)]
use std::thread;
use std::time::Duration;

mod device;
mod protocol;
mod transfer;

#[derive(Parser)]
#[command(
    version,
    about = "Forward a host USB device to a Linux microVM over USB/IP"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List USB bus IDs and interface descriptors without claiming devices.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Serve one device on a private Unix socket, in the foreground.
    /// Supports control, bulk and interrupt transfers; no MTP software is used.
    Serve {
        #[arg(long)]
        device: String,
        #[arg(long)]
        socket: PathBuf,
    },
}

fn serve_client(
    mut stream: UnixStream,
    context: &rusb::Context,
    id: &str,
    stopped: &AtomicBool,
) -> anyhow::Result<()> {
    // Accepted sockets can inherit the listener's nonblocking flag on macOS.
    stream
        .set_nonblocking(false)
        .context("Configure blocking USB/IP session")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .context("Set USB/IP handshake read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .context("Set USB/IP handshake write timeout")?;
    match protocol::read_operation(&mut stream)? {
        protocol::DEVLIST => {
            let device = device::find(context, id)?;
            let info = device::DeviceInfo::read(&device)?;
            protocol::write_operation(&mut stream, protocol::DEVLIST, 0)?;
            stream
                .write_all(&1u32.to_be_bytes())
                .context("Write USB/IP device count")?;
            protocol::write_device(&mut stream, &info, true)
        }
        protocol::IMPORT => {
            if protocol::read_bus_id(&mut stream)? != id {
                return protocol::write_operation(&mut stream, protocol::IMPORT, 4);
            }
            let device = match device::find(context, id) {
                Ok(device) => device,
                Err(error) => {
                    protocol::write_operation(&mut stream, protocol::IMPORT, 4)?;
                    return Err(error);
                }
            };
            let info = device::DeviceInfo::read(&device)?;
            let opened = match device::UsbDevice::open(&device) {
                Ok(opened) => opened,
                Err(error) => {
                    protocol::write_operation(&mut stream, protocol::IMPORT, 3)?;
                    return Err(error);
                }
            };
            protocol::write_operation(&mut stream, protocol::IMPORT, 0)?;
            protocol::write_device(&mut stream, &info, false)?;
            transfer::forward(opened, stream, stopped)
        }
        operation => anyhow::bail!("Unsupported USB/IP operation: {operation:#x}"),
    }
}

fn serve(
    context: &rusb::Context,
    id: &str,
    socket: PathBuf,
    sudo_uid: Option<u32>,
    sudo_gid: Option<u32>,
) -> anyhow::Result<()> {
    let selected = device::find(context, id)?;
    // Keep the descriptor alive for the helper's lifetime, like the host's disk
    // locks. Never unlink a live lock file: another helper could lock a new inode.
    let _device_lock = lock_device(id)?;
    let stopped = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, stopped.clone())
            .context("Register USB helper termination signal")?;
    }
    let _removal = rusb::HotplugBuilder::new()
        .register(
            context,
            Box::new(DeviceRemoval {
                bus: selected.bus_number(),
                address: selected.address(),
                stopped: stopped.clone(),
            }),
        )
        .context("Watch selected USB device removal")?;
    let listener = UnixListener::bind(&socket)
        .with_context(|| format!("Bind USB socket {}", socket.display()))?;
    let mut cleanup = Deferred::new();
    cleanup.add(|| {
        if let Err(error) = fs::remove_file(&socket)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("Remove USB socket: {error}");
        }
    });
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
        .context("Restrict USB socket permissions")?;
    if let (Some(uid), Some(gid)) = (sudo_uid, sudo_gid) {
        chown(&socket, Some(uid), Some(gid)).context("Set USB socket owner to invoking user")?;
    }
    listener
        .set_nonblocking(true)
        .context("Configure USB socket listener")?;
    println!("{}", serde_json::json!({ "device": id, "socket": socket }));
    io::stdout().flush().context("Flush USB helper readiness")?;
    while !stopped.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                if let Err(error) = serve_client(stream, context, id, &stopped) {
                    eprintln!("USB/IP session: {error:#}");
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                match context.handle_events(Some(Duration::from_millis(50))) {
                    Ok(()) | Err(rusb::Error::Interrupted) => {}
                    Err(error) => return Err(error).context("Watch USB device events"),
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error).context("Accept USB/IP connection"),
        }
    }
    Ok(())
}

struct DeviceRemoval {
    bus: u8,
    address: u8,
    stopped: Arc<AtomicBool>,
}

impl rusb::Hotplug<rusb::Context> for DeviceRemoval {
    fn device_arrived(&mut self, _device: rusb::Device<rusb::Context>) {}

    fn device_left(&mut self, device: rusb::Device<rusb::Context>) {
        if device.bus_number() == self.bus && device.address() == self.address {
            self.stopped.store(true, Ordering::Relaxed);
        }
    }
}

fn lock_device(id: &str) -> anyhow::Result<fs::File> {
    let path = PathBuf::from(format!("/tmp/anylinuxfs-usb-{id}.lock"));
    lock_file(&path).with_context(|| format!("Lock USB device {id}"))
}

fn lock_file(path: &Path) -> anyhow::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = match options.create_new(true).mode(0o666).open(path) {
        Ok(file) => {
            file.set_permissions(fs::Permissions::from_mode(0o666))
                .context("Set USB device lock permissions")?;
            file
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => options
            .create_new(false)
            .open(path)
            .context("Open USB device lock")?,
        Err(error) => return Err(error).context("Create USB device lock"),
    };
    if !file
        .metadata()
        .context("Read USB device lock metadata")?
        .is_file()
    {
        anyhow::bail!("USB device lock is not a regular file");
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } < 0 {
        return Err(io::Error::last_os_error())
            .context("USB device is already served or cannot be locked");
    }
    Ok(file)
}

fn run() -> anyhow::Result<()> {
    let sudo_uid = std::env::var("SUDO_UID")
        .ok()
        .map(|v| v.parse())
        .transpose()
        .context("Parse SUDO_UID")?;
    let sudo_gid = std::env::var("SUDO_GID")
        .ok()
        .map(|v| v.parse())
        .transpose()
        .context("Parse SUDO_GID")?;
    let cli = Cli::parse();
    let context = rusb::Context::new().context("Initialize bundled USB library")?;
    match cli.command {
        Command::List { json } => {
            let mut devices = Vec::new();
            for device in context.devices().context("Enumerate USB devices")?.iter() {
                match device::DeviceInfo::read(&device).and_then(|info| {
                    let speed = protocol::speed_value(info.speed)?;
                    Ok((info, speed))
                }) {
                    Ok(info) => devices.push(info),
                    Err(error) => {
                        eprintln!("Read USB device {}: {error:#}", device::bus_id(&device))
                    }
                }
            }
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&devices.iter().map(|(info, speed)| serde_json::json!({
                        "bus_id": info.bus_id, "vendor_id": info.vendor_id, "product_id": info.product_id,
                        "speed": speed, "interfaces": info.interfaces,
                    })).collect::<Vec<_>>()).context("Serialize USB device list")?
                );
            } else {
                for (info, speed) in devices {
                    println!(
                        "{} {:04x}:{:04x} speed={} interfaces={}",
                        info.bus_id,
                        info.vendor_id,
                        info.product_id,
                        speed,
                        info.interfaces
                            .iter()
                            .map(|i| format!(
                                "{}:{:02x}/{:02x}/{:02x}",
                                i.number, i.class, i.subclass, i.protocol
                            ))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
            }
            Ok(())
        }
        Command::Serve { device, socket } => serve(&context, &device, socket, sudo_uid, sudo_gid),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    #[test]
    fn accepted_nonblocking_socket_reads_fragmented_handshake() {
        let context = rusb::Context::new().unwrap();
        let (server, mut client) = UnixStream::pair().unwrap();
        server.set_nonblocking(true).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        thread::scope(|scope| {
            let writer = scope.spawn(move || {
                client.write_all(&[1, 17, 128, 3, 0, 0, 0, 0]).unwrap();
                thread::sleep(Duration::from_millis(20));
                client.write_all(&[0; 32]).unwrap();
                let mut response = [0; 8];
                client.read_exact(&mut response).unwrap();
                assert_eq!(response, [1, 17, 0, 3, 0, 0, 0, 4]);
            });
            serve_client(server, &context, "1-2", &AtomicBool::new(false)).unwrap();
            writer.join().unwrap();
        });
    }
}
