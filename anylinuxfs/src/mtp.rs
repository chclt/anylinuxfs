//! Host resources for an MTP mount. USB forwarding has its own lifecycle.
use anyhow::Context;
use common_utils::{Deferred, host_eprintln};
use std::{ffi::OsStr, fs, io, os::unix::fs::DirBuilderExt};

use crate::{
    privilege,
    settings::{Config, MtpConfig},
};

pub(crate) fn prepare_cache(
    config: &Config,
    mtp: &MtpConfig,
    deferred: &mut Deferred<'_>,
) -> anyhow::Result<()> {
    let cache = &mtp.cache_path;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(cache)
        .context("Create MTP disk cache")?;
    deferred.add({
        let cache = cache.clone();
        move || {
            if let Err(error) = cleanup_cache(&cache) {
                host_eprintln!("Remove MTP disk cache {}: {}", cache.display(), error);
            }
        }
    });
    privilege::chown_to_invoker(
        cache,
        config.privilege.invoker_uid,
        config.privilege.invoker_gid,
    )?;
    Ok(())
}

pub(crate) fn cleanup_cache(cache: &std::path::Path) -> anyhow::Result<()> {
    let writeback = cache.join(common_utils::MTP_WRITEBACK_DIR);
    match fs::read_dir(&writeback) {
        Ok(entries) => {
            for entry in entries {
                let path = entry.context("Read MTP writeback entry")?.path();
                if matches!(path.extension(), Some(extension) if extension == OsStr::new("record") || extension == OsStr::new("next"))
                {
                    host_eprintln!(
                        "MTP writeback needs recovery; cache retained at {}",
                        cache.display()
                    );
                    return Ok(());
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Read MTP writeback directory"),
    }
    match fs::remove_dir_all(cache) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Remove MTP cache"),
    }
}
