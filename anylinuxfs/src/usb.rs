//! Host-side ownership of the USB helper. The guest handles USB/IP attachment
//! and filesystem protocols; this module only starts and cleans up forwarding.
use anyhow::Context;
use common_utils::{Deferred, host_eprintln, host_println};
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::{self, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
};

use crate::{
    privilege,
    settings::{Config, UsbConfig},
};

pub(crate) fn list(config: &Config) -> anyhow::Result<()> {
    let mut command = Command::new(config.paths.libexec_path.join("usbproxy"));
    command.arg("list");
    privilege::run_as_invoker(
        &mut command,
        config.privilege.sudo_uid,
        config.privilege.sudo_gid,
    );
    let status = command.status().context("List host USB devices")?;
    if !status.success() {
        anyhow::bail!("USB device listing failed: {status}");
    }
    Ok(())
}

pub(crate) fn start(
    config: &Config,
    usb: &UsbConfig,
    deferred: &mut Deferred<'_>,
) -> anyhow::Result<libc::pid_t> {
    let log_path = &config.logs.usbproxy_log_path;
    let log = File::create(log_path).context("Create USB helper log")?;
    privilege::chown_to_invoker(
        log_path,
        config.privilege.invoker_uid,
        config.privilege.invoker_gid,
    )?;
    let mut command = Command::new(config.paths.libexec_path.join("usbproxy"));
    command
        .arg("serve")
        .arg("--device")
        .arg(&usb.device)
        .arg("--socket")
        .arg(&usb.socket_path)
        .stdout(Stdio::piped())
        .stderr(log);
    // Linux USB device nodes may require root; macOS forwarding runs as the invoker.
    #[cfg(target_os = "macos")]
    privilege::run_as_invoker(
        &mut command,
        config.privilege.sudo_uid,
        config.privilege.sudo_gid,
    );
    let mut child = command.spawn().context("Start USB forwarding helper")?;
    let pid = child.id() as libc::pid_t;
    let stdout = child.stdout.take();
    deferred.add({
        let socket = usb.socket_path.clone();
        move || {
            if let Err(error) = cleanup_socket(&socket) {
                host_eprintln!("{:#}", error);
            }
        }
    });
    deferred.add(move || {
        stop_child(&mut child);
    });
    let ready = read_ready(stdout.context("Capture USB helper readiness")?).with_context(|| {
        format!(
            "Start USB forwarding for {}; see {}",
            usb.device,
            log_path.display()
        )
    })?;
    host_println!(
        "USB device {} forwarded through {}",
        ready.device,
        ready.socket.display()
    );
    Ok(pid)
}

#[derive(Deserialize)]
struct Ready {
    device: String,
    socket: std::path::PathBuf,
}

fn read_ready(stdout: std::process::ChildStdout) -> anyhow::Result<Ready> {
    let reader = BufReader::new(stdout);
    // Deserialize one value: the helper keeps stdout open while serving the VM.
    Ready::deserialize(&mut serde_json::Deserializer::from_reader(reader))
        .context("Read USB helper readiness")
}

fn stop_child(child: &mut Child) {
    match child.try_wait() {
        Ok(Some(_)) => {}
        Ok(None) => {
            if let Err(error) = common_utils::terminate_child(
                child,
                "usbproxy",
                Some(common_utils::log::Prefix::Host),
            ) {
                host_eprintln!("{:#}", error);
            }
        }
        Err(error) => host_eprintln!("Check USB helper status: {}", error),
    }
}

pub(crate) fn cleanup_socket(socket: &Path) -> anyhow::Result<()> {
    match fs::remove_file(socket) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Remove USB forwarding socket"),
    }
}
