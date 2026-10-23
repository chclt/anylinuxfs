//! Set ALFS_USB_TRACE to enable transport metadata logs on stderr.
//! Keep stdout for the readiness handshake; never log transfer payloads.
use std::{fmt, sync::OnceLock, time::SystemTime};

pub(crate) fn event(message: fmt::Arguments<'_>) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("ALFS_USB_TRACE").is_some()) {
        return;
    }
    let millis = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    eprintln!("USB trace time_ms={millis} {message}");
}
