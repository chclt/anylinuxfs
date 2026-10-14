//! NFS exports consume mounted filesystem paths and permissions.
use anyhow::Context;
#[cfg(target_os = "linux")]
use common_utils::FromPath;
use std::{collections::BTreeSet, fs};
#[cfg(target_os = "linux")]
use std::{ffi::CString, io, path::Path};

// TODO: we might need this for custom actions on FreeBSD
#[cfg(target_os = "linux")]
fn statfs(path: impl AsRef<Path>) -> io::Result<libc::statfs> {
    let c_path = CString::from_path(path.as_ref());
    let mut buf: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c_path.as_ptr(), &mut buf) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buf)
}

fn export_args_for_path(
    _path: &str,
    export_mode: &str,
    _fsid: usize,
    export_args_override: Option<&str>,
) -> anyhow::Result<String> {
    #[cfg(target_os = "linux")]
    let mut export_args = if let Some(override_args) = export_args_override {
        override_args.to_owned()
    } else {
        format!("{export_mode},no_subtree_check,no_root_squash,insecure")
    };
    #[cfg(any(target_os = "freebsd", target_os = "macos"))]
    let export_args = if let Some(override_args) = export_args_override {
        override_args.to_owned()
    } else {
        format!(
            "{}-maproot=root",
            if export_mode == "ro" { "-ro " } else { "" }
        )
    };

    #[cfg(target_os = "linux")]
    if statfs(_path)
        .with_context(|| format!("statfs failed for {_path}"))?
        .f_type
        == 0x65735546
    {
        // exporting FUSE requires fsid
        if !export_args.contains("fsid=") {
            export_args += &format!(",fsid={}", _fsid)
        }
    }
    Ok(export_args)
}

pub(crate) fn export(
    paths: Vec<String>,
    read_only: bool,
    override_options: Option<&str>,
    ignore_permissions: bool,
) -> anyhow::Result<()> {
    let mode = if read_only { "ro" } else { "rw" };
    let squash_options;
    let options = if ignore_permissions && override_options.is_none() {
        squash_options = format!("{mode},no_subtree_check,all_squash,anonuid=0,anongid=0,insecure");
        Some(squash_options.as_str())
    } else {
        override_options
    };
    let paths: BTreeSet<_> = paths.into_iter().collect();
    let mut content = String::new();
    for (index, path) in paths.into_iter().enumerate() {
        let args = export_args_for_path(&path, mode, index, options)?;
        println!("<anylinuxfs-nfs-export:{path}>");
        #[cfg(target_os = "linux")]
        {
            content += &format!("\"{path}\"      *({args})\n");
        }
        #[cfg(any(target_os = "freebsd", target_os = "macos"))]
        {
            content += &format!("{path} {args},network 0.0.0.0/0\n");
        }
    }
    let file = if cfg!(target_os = "freebsd") {
        "/etc/exports"
    } else {
        "/tmp/exports"
    };
    fs::write(file, content).with_context(|| format!("Failed to write to {file}"))?;
    println!("Successfully initialized {file}.");
    Ok(())
}
