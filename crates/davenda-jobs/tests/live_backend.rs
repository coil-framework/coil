use davenda_config::JobBackend;
use davenda_jobs::{JobsBackendAdapter, JobsModelError, JobsRuntime};
use std::path::PathBuf;

fn config(backend: JobBackend) -> davenda_config::JobsConfig {
    davenda_config::JobsConfig {
        backend,
        retry_limit: 3,
    }
}

#[test]
fn live_shared_runtime_returns_error_for_invalid_database_url() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let original_database_url = std::env::var("DATABASE_URL").ok();

    unsafe {
        std::env::set_var("DATABASE_URL", "not-a-valid-database-url");
    }

    let result =
        JobsBackendAdapter::live_shared_runtime(&runtime, "jobs-live-backend-test", PathBuf::new());

    assert!(matches!(
        result,
        Err(JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
            backend: JobBackend::Redis,
            ..
        })
    ));

    if let Some(database_url) = original_database_url {
        unsafe {
            std::env::set_var("DATABASE_URL", database_url);
        }
    } else {
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
    }
}
