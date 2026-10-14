//! Host resources for an MTP mount. USB forwarding has its own lifecycle.
use anyhow::Context;
use common_utils::{Deferred, host_eprintln};
use std::{fs, os::unix::fs::DirBuilderExt};

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
            if let Err(error) = fs::remove_dir_all(&cache) {
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
