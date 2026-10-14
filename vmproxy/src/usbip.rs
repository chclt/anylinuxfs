//! Import the host's USB device into Linux VHCI. Filesystem protocols stay separate.
use anyhow::{Context, bail};

const USBIP_VERSION: u16 = 0x0111;
const IMPORT_REPLY: u16 = 0x0003;
#[cfg(target_os = "linux")]
const VHCI: &str = "/sys/devices/platform/vhci_hcd.0";

fn import_request(bus_id: &str) -> anyhow::Result<[u8; 40]> {
    if bus_id.is_empty() || bus_id.len() >= 32 || bus_id.as_bytes().contains(&0) {
        bail!("USB/IP bus ID must contain 1–31 bytes and no NUL bytes");
    }
    let mut request = [0; 40];
    request[..2].copy_from_slice(&USBIP_VERSION.to_be_bytes());
    request[2..4].copy_from_slice(&0x8003u16.to_be_bytes());
    request[8..8 + bus_id.len()].copy_from_slice(bus_id.as_bytes());
    Ok(request)
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn import_reply(device: &[u8; 312]) -> anyhow::Result<(u32, u32)> {
    // Linux usbip_usb_device: path[256], busid[32], busnum, devnum, speed.
    let bus = be_u32(&device[288..292]);
    let number = be_u32(&device[292..296]);
    let speed = be_u32(&device[296..300]);
    if bus > u16::MAX as u32 || number == 0 || number > u16::MAX as u32 {
        bail!("Invalid USB/IP device address: bus={bus}, device={number}");
    }
    Ok(((bus << 16) | number, speed))
}

fn check_header(header: &[u8; 8]) -> anyhow::Result<()> {
    let version = u16::from_be_bytes([header[0], header[1]]);
    let operation = u16::from_be_bytes([header[2], header[3]]);
    let status = be_u32(&header[4..8]);
    if version != USBIP_VERSION || operation != IMPORT_REPLY {
        bail!("Invalid USB/IP import reply: version={version:#x}, operation={operation:#x}");
    }
    if status != 0 {
        bail!("Host USB bridge rejected import: status={status}");
    }
    Ok(())
}

fn free_port(status: &str, speed: u32) -> anyhow::Result<u16> {
    let hub = match speed {
        1..=3 => "hs",
        5..=6 => "ss",
        _ => bail!("Unsupported USB/IP device speed: {speed}"),
    };
    for line in status.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() >= 3 && fields[0] == hub {
            let state: u32 = fields[2].parse().context("Invalid VHCI port state")?;
            // VDEV_ST_NULL in Linux usbip_common.h.
            if state == 4 {
                return fields[1].parse().context("Invalid VHCI port number");
            }
        }
    }
    bail!("No free {hub} USB/IP port in the guest")
}

fn local_bus_id(status: &str, port: u16) -> anyhow::Result<Option<&str>> {
    for line in status.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() >= 7 && fields[1].parse::<u16>().ok() == Some(port) {
            match fields[2]
                .parse::<u32>()
                .context("Invalid VHCI port state")?
            {
                6 => return Ok(Some(fields[6])), // VDEV_ST_USED
                7 => bail!("USB/IP enumeration failed on guest port {port}"),
                _ => return Ok(None),
            }
        }
    }
    bail!("Guest VHCI port {port} disappeared")
}

#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(crate) struct Device {
    pub bus: u32,
    pub number: u32,
}

#[cfg(target_os = "linux")]
pub(crate) fn attach(
    bus_id: &str,
    deferred: &mut common_utils::Deferred<'_>,
) -> anyhow::Result<Device> {
    use std::{
        fs,
        io::{Read, Write},
        os::fd::AsRawFd,
        thread,
        time::{Duration, Instant},
    };
    use vsock::VsockStream;

    let request = import_request(bus_id)?;
    let status = fs::read_to_string(format!("{VHCI}/status"))
        .context("Guest kernel needs CONFIG_USBIP_VHCI_HCD; use Image-usbip")?;
    let mut stream = VsockStream::connect_with_cid_port(2, common_utils::USBIP_VSOCK_PORT)
        .context("Connect to host USB bridge")?;
    let timeout = Some(Duration::from_secs(10));
    stream
        .set_read_timeout(timeout)
        .context("Set USB/IP handshake read timeout")?;
    stream
        .set_write_timeout(timeout)
        .context("Set USB/IP handshake write timeout")?;
    stream
        .write_all(&request)
        .context("Send USB/IP import request")?;
    let mut header = [0; 8];
    stream
        .read_exact(&mut header)
        .context("Read USB/IP import reply")?;
    check_header(&header)?;
    let mut metadata = [0; 312];
    stream
        .read_exact(&mut metadata)
        .context("Read USB/IP device metadata")?;
    let (device_id, speed) = import_reply(&metadata)?;
    let port = free_port(&status, speed)?;
    // VHCI retains the socket reference after our descriptor closes.
    stream
        .set_read_timeout(None)
        .context("Clear USB/IP read timeout")?;
    stream
        .set_write_timeout(None)
        .context("Clear USB/IP write timeout")?;
    fs::write(
        format!("{VHCI}/attach"),
        format!("{port} {} {device_id} {speed}", stream.as_raw_fd()),
    )
    .context("Attach USB/IP stream to VHCI")?;
    deferred.add(move || {
        if let Err(error) = fs::write(format!("{VHCI}/detach"), port.to_string()) {
            eprintln!("Detach USB/IP port {port}: {error}");
        }
    });

    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let status =
            fs::read_to_string(format!("{VHCI}/status")).context("Read VHCI attachment status")?;
        if let Some(bus_id) = local_bus_id(&status, port)? {
            let path = format!("/sys/bus/usb/devices/{bus_id}");
            let address = fs::read_to_string(format!("{path}/busnum")).and_then(|bus| {
                fs::read_to_string(format!("{path}/devnum")).map(|number| (bus, number))
            });
            match address {
                Ok((bus, number)) => {
                    return Ok(Device {
                        bus: bus.trim().parse().context("Invalid guest USB bus number")?,
                        number: number
                            .trim()
                            .parse()
                            .context("Invalid guest USB device address")?,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error).context("Read imported guest USB address"),
            }
        }
        if Instant::now() >= deadline {
            bail!("Timed out enumerating USB/IP device {bus_id}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const STATUS: &str = "hub port sta spd dev sockfd local_busid\nhs 0000 006 003 00010002 3 1-1\nhs 0001 004 000 00000000 0 0-0\nss 0008 004 000 00000000 0 0-0\n";

    #[test]
    fn selects_free_port_for_device_speed() {
        assert_eq!(free_port(STATUS, 3).unwrap(), 1);
        assert_eq!(free_port(STATUS, 5).unwrap(), 8);
        assert!(free_port(STATUS, 4).is_err());
        assert!(free_port(&STATUS.replace("004", "006"), 3).is_err());
    }

    #[test]
    fn enumeration_uses_guest_address_and_waits_until_assigned() {
        assert_eq!(local_bus_id(STATUS, 0).unwrap(), Some("1-1"));
        assert_eq!(local_bus_id(STATUS, 1).unwrap(), None);
        assert!(local_bus_id(&STATUS.replace("006", "007"), 0).is_err());
    }

    #[test]
    fn import_packet_and_reply_follow_linux_layout() {
        let request = import_request("1-2").unwrap();
        assert_eq!(&request[..8], &[1, 17, 128, 3, 0, 0, 0, 0]);
        assert_eq!(&request[8..12], b"1-2\0");
        assert!(import_request("").is_err());
        assert!(import_request(&"a".repeat(32)).is_err());
        assert!(import_request("1-2\0").is_err());
        let mut device = [0; 312];
        device[288..292].copy_from_slice(&1u32.to_be_bytes());
        device[292..296].copy_from_slice(&2u32.to_be_bytes());
        device[296..300].copy_from_slice(&3u32.to_be_bytes());
        assert_eq!(import_reply(&device).unwrap(), (0x10002, 3));
        assert!(check_header(&[1, 17, 0, 3, 0, 0, 0, 1]).is_err());
        assert!(check_header(&[1, 17, 0, 5, 0, 0, 0, 0]).is_err());
        device[292..296].fill(0);
        assert!(import_reply(&device).is_err());
    }
}
