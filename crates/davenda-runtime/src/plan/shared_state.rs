use super::*;
use std::path::PathBuf;

pub(crate) fn shared_state_root(config: &PlatformConfig) -> PathBuf {
    std::env::var_os("DAVENDA_SHARED_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(&config.storage.local_root)
                .join("shared-state")
        })
}
