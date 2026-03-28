use std::time::Duration;

use coil_jobs::RetryPolicy;

pub fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}
