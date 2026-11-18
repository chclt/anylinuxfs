//! Read-only host resources for AFT. File data is transferred directly by MTP.
use anyhow::Context;
use common_utils::{Deferred, host_eprintln};
use std::{fs, io, os::unix::fs::DirBuilderExt, path::Path};

use crate::{
    privilege,
    settings::{Config, MtpConfig},
};

pub(crate) fn prepare_resources(
    config: &Config,
    mtp: &MtpConfig,
    deferred: &mut Deferred<'_>,
) -> anyhow::Result<()> {
    let path = &mtp.resources_path;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .context("Create MTP resources directory")?;
    deferred.add({
        let path = path.clone();
        move || {
            if let Err(error) = cleanup_resources(&path) {
                host_eprintln!("Remove MTP resources {}: {:#}", path.display(), error);
            }
        }
    });
    // MTP dates often omit a zone. Copy the host's TZif rules, including DST,
    // and give only AFT this timezone; the guest's system timezone is unchanged.
    fs::copy("/etc/localtime", path.join("localtime")).context("Copy host timezone for MTP")?;
    privilege::chown_to_invoker(
        path,
        config.privilege.invoker_uid,
        config.privilege.invoker_gid,
    )?;
    Ok(())
}

pub(crate) fn cleanup_resources(path: &Path) -> anyhow::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Remove MTP resources"),
    }
}
