use super::*;

pub(crate) fn unconfigured_live_jobs_error(
    backend: davenda_config::JobBackend,
) -> RuntimeJobsError {
    RuntimeJobsError::LiveSharedRuntimeRequiresExplicitBackend { backend }
}

pub(crate) fn unconfigured_live_cache_error(
    kind: davenda_cache::CacheBackendKind,
) -> RuntimeCacheError {
    RuntimeCacheError::LiveSharedRuntimeRequiresExplicitBackend { kind }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_jobs_error_is_typed() {
        let error = unconfigured_live_jobs_error(davenda_config::JobBackend::Redis);
        assert_eq!(
            error,
            RuntimeJobsError::LiveSharedRuntimeRequiresExplicitBackend {
                backend: davenda_config::JobBackend::Redis,
            }
        );
        assert!(error.to_string().contains("explicit distributed runtime"));
    }

    #[test]
    fn live_cache_error_is_typed() {
        let error = unconfigured_live_cache_error(davenda_cache::CacheBackendKind::Redis);
        assert_eq!(
            error,
            RuntimeCacheError::LiveSharedRuntimeRequiresExplicitBackend {
                kind: davenda_cache::CacheBackendKind::Redis,
            }
        );
        assert!(
            error
                .to_string()
                .contains("file-backed shared state is test-only")
        );
    }
}
