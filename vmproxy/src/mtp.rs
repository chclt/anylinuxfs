//! MTP mount lifecycle. USB transport and NFS exports have their own owners.
#[cfg(target_os = "linux")]
use anyhow::Context;
use common_utils::Deferred;

#[cfg(any(target_os = "linux", test))]
fn mount_ready(mountinfo: &str, mount_point: &str, mode: &str) -> bool {
    let escaped = mount_point
        .replace('\\', "\\134")
        .replace(' ', "\\040")
        .replace('\t', "\\011")
        .replace('\n', "\\012");
    mountinfo.lines().any(|line| {
        let Some((mount, filesystem)) = line.split_once(" - ") else {
            return false;
        };
        let fields: Vec<_> = mount.split_whitespace().collect();
        fields.len() >= 6
            && fields[4] == escaped
            && fields[5].split(',').any(|option| option == mode)
            && matches!(
                filesystem.split_whitespace().next(),
                Some("fuse" | "fuse.jmtpfs")
            )
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn mount(
    bus_id: &str,
    mount_point: &str,
    read_only: bool,
    owner: (u32, u32),
    deferred: &mut Deferred<'_>,
) -> anyhow::Result<()> {
    use std::{
        fs,
        process::Command,
        thread,
        time::{Duration, Instant},
    };
    const DRIVER: &str = "/usr/local/libexec/anylinuxfs/jmtpfs-writeback-v1";
    fs::metadata(DRIVER).context(
        "MTP writeback helper v1 is missing; run scripts/prepare-mtp-guest.sh inside the Linux guest",
    )?;
    fs::metadata("/dev/fuse").context("Guest kernel needs CONFIG_FUSE_FS")?;

    let device = crate::usbip::attach(bus_id, deferred).context("Attach MTP USB device")?;
    let mode = if read_only { "ro" } else { "rw" };
    // libfuse otherwise derives the subtype from the versioned helper filename,
    // which would no longer match the stable type checked by mount_ready.
    let options = format!(
        "{mode},subtype=jmtpfs,allow_other,noforget,uid={},gid={}",
        owner.0, owner.1
    );
    let child = Command::new(DRIVER)
        .args(["-f", "-s", "-o", &options])
        .env(
            "JMTPFS_WRITEBACK_DIR",
            std::path::Path::new("/tmp").join(common_utils::MTP_WRITEBACK_DIR),
        )
        .arg(format!("-device={},{}", device.bus, device.number))
        .arg(mount_point)
        .spawn()
        .context("Start MTP filesystem")?;

    let child = std::cell::RefCell::new(child);
    let mut cleanup = Deferred::new();
    cleanup.add(|| stop(&mut child.borrow_mut(), mount_point));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child
            .borrow_mut()
            .try_wait()
            .context("Check MTP helper status")?
        {
            anyhow::bail!(
                "MTP helper exited before mounting: {status}; unlock the device and enable file transfer"
            );
        }
        // Reading mountinfo does not issue FUSE requests that can block on a phone.
        let mounts = fs::read_to_string("/proc/self/mountinfo").context("Read MTP mount status")?;
        if mount_ready(&mounts, mount_point, mode) {
            break;
        }
        if Instant::now() >= deadline {
            anyhow::bail!(
                "Timed out mounting MTP device {bus_id}; unlock it and enable file transfer"
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
    cleanup.remove_all();
    drop(cleanup);
    let mut child = child.into_inner();
    let mount_point = mount_point.to_owned();
    deferred.add(move || stop(&mut child, &mount_point));
    println!("MTP device {bus_id} mounted {mode}.");
    Ok(())
}

#[cfg(target_os = "linux")]
fn stop(child: &mut std::process::Child, mount_point: &str) {
    use sys_mount::{UnmountFlags, unmount};
    // NFS has stopped before this cleanup. Fall back to detaching if the FUSE
    // daemon disconnected, then reap it before the USB port is released.
    let unmounted = match unmount(mount_point, UnmountFlags::empty()) {
        Ok(()) => true,
        Err(error) if error.raw_os_error() == Some(libc::EINVAL) => false,
        Err(error) => {
            eprintln!("Unmount MTP filesystem: {error}");
            match unmount(mount_point, UnmountFlags::DETACH) {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("Detach MTP filesystem: {error}");
                    false
                }
            }
        }
    };
    // fuse_main removes its signal handlers before the jmtpfs context closes
    // the MTP session. Once unmounted, let that cleanup finish without SIGTERM.
    // The shared wait bounds cleanup before USB/IP is detached. If unmount did
    // not succeed (including startup failure), explicitly stop any live helper.
    let result = if unmounted {
        common_utils::wait_for_child(child, "jmtpfs", None)
    } else {
        match child.try_wait() {
            Ok(Some(_)) => Ok(()),
            Ok(None) => crate::terminate_child(child, "jmtpfs"),
            Err(error) => Err(error).context("Check MTP helper status"),
        }
    };
    if let Err(error) = result {
        eprintln!("{error:#}");
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn mount(
    _bus_id: &str,
    _mount_point: &str,
    _read_only: bool,
    _owner: (u32, u32),
    _deferred: &mut Deferred<'_>,
) -> anyhow::Result<()> {
    anyhow::bail!("MTP requires a Linux guest")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_requires_requested_mode_and_fuse_at_exact_mount_point() {
        let line = "31 20 0:33 / /mnt/My\\040Phone ro,nosuid - fuse.jmtpfs jmtpfs rw,user_id=0";
        assert!(mount_ready(line, "/mnt/My Phone", "ro"));
        assert!(!mount_ready(line, "/mnt/My", "ro"));
        assert!(!mount_ready(
            &line.replace("ro,nosuid", "rw,nosuid"),
            "/mnt/My Phone",
            "ro"
        ));
        assert!(mount_ready(
            &line.replace("ro,nosuid", "rw,nosuid"),
            "/mnt/My Phone",
            "rw"
        ));
        assert!(!mount_ready(
            &line.replace("fuse.jmtpfs", "virtiofs"),
            "/mnt/My Phone",
            "ro"
        ));
    }
}
