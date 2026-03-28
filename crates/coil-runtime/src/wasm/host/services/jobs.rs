use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use coil_jobs::{JobId, JobInstant, JobName, JobQueueName, JobSpec};
use coil_wasm::JobExecution;

use super::super::*;

#[derive(Debug, Clone)]
pub(super) struct RuntimeJobBackend {
    plan: RuntimePlan,
    sequence: Arc<AtomicU64>,
}

impl RuntimeJobBackend {
    pub(super) fn new(plan: RuntimePlan) -> Self {
        Self {
            plan,
            sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn enqueue(
        &self,
        queue: &str,
        context: &InvocationContext,
    ) -> Result<JobExecution, String> {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let queue = JobQueueName::new(queue.to_string()).map_err(|error| error.to_string())?;
        let job_id = JobId::new(format!("wasm:{}:{}:{}", context.trace.trace_id, queue, seq))
            .map_err(|error| error.to_string())?;
        let job_name =
            JobName::new(format!("wasm.enqueue.{queue}")).map_err(|error| error.to_string())?;
        let payload_description = format!(
            "wasm host enqueue for queue `{queue}` from trace `{}`",
            context.trace.trace_id
        );
        let spec = JobSpec::new(job_id.clone(), job_name, queue.clone(), payload_description)
            .map_err(|error| error.to_string())?;

        let now = current_job_instant();
        let mut host = self
            .plan
            .jobs_host("wasm-host")
            .map_err(|error| error.to_string())?;
        let _ = host
            .enqueue_spec(spec, now)
            .map_err(|error| error.to_string())?;

        Ok(JobExecution {
            queue: queue.to_string(),
            job_id: job_id.to_string(),
            enqueued_at_unix_seconds: now.as_unix_seconds(),
        })
    }
}

fn current_job_instant() -> JobInstant {
    JobInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    )
}
