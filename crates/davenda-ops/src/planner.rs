use crate::bulk::{BulkOperationPlan, BulkOperationRequest};
use crate::catalog::OpsCatalog;
use crate::error::OpsModelError;
use crate::reports::{ReportExportPlan, ReportExportRequest};
use davenda_jobs::{JobId, JobName, JobSpec, JobsPlanner, JobsRuntime, RetryPolicy};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsPlanner {
    jobs: JobsPlanner,
    catalog: OpsCatalog,
}

impl OpsPlanner {
    pub fn new(runtime: JobsRuntime, catalog: OpsCatalog) -> Result<Self, OpsModelError> {
        catalog.validate()?;
        Ok(Self {
            jobs: runtime.planner(),
            catalog,
        })
    }

    pub fn jobs_planner(&self) -> &JobsPlanner {
        &self.jobs
    }

    pub fn catalog(&self) -> &OpsCatalog {
        &self.catalog
    }

    pub fn plan_report_export(
        &self,
        request: ReportExportRequest,
    ) -> Result<ReportExportPlan, OpsModelError> {
        let definition = self
            .catalog
            .reports
            .definition(&request.report_id)
            .ok_or_else(|| OpsModelError::DuplicateIdentifier {
                kind: "report",
                id: request.report_id.to_string(),
            })?;

        if !definition.allows(&request.operator_capabilities) {
            return Err(OpsModelError::MissingCapability {
                operation: "report export",
                required: definition.required_capability,
            });
        }

        let queue_topology = self.jobs.describe_queue_topology();
        let queue = if request.scheduled_for.is_some() {
            queue_topology.scheduled_queue.clone()
        } else {
            queue_topology.work_queue.clone()
        };

        let job_id = JobId::new(request.export_id.as_str().to_string())?;
        let job_name = JobName::new(format!("report.export.{}", definition.id.as_str()))?;
        let mut retry_policy = definition.retry_policy.clone();
        retry_policy =
            retry_policy.with_dead_letter_queue(queue_topology.dead_letter_queue.clone());

        let mut spec = JobSpec::new(
            job_id,
            job_name,
            queue,
            format!(
                "report export for `{}` in format `{}`",
                definition.id, definition.format
            ),
        )?;
        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }
        spec = spec.with_retry_policy(retry_policy);
        if let Some(key) = request.idempotency_key.clone() {
            spec = spec.with_idempotency_key(key);
        }

        let planned_job = self.jobs.plan_job(spec.clone(), request.requested_at)?;

        Ok(ReportExportPlan {
            definition: definition.clone(),
            job: spec,
            planned_job,
            output_object_key: format!(
                "{}/{}.{}",
                definition.export_prefix,
                request.export_id,
                definition.format.extension()
            ),
            parameters: request.parameters,
        })
    }

    pub fn plan_bulk_operation(
        &self,
        request: BulkOperationRequest,
    ) -> Result<BulkOperationPlan, OpsModelError> {
        let definition = self
            .catalog
            .bulk
            .definition(&request.definition_id)
            .ok_or_else(|| OpsModelError::DuplicateIdentifier {
                kind: "bulk operation",
                id: request.definition_id.to_string(),
            })?;

        if !definition.allows(&request.operator_capabilities) {
            return Err(OpsModelError::MissingCapability {
                operation: "bulk operation",
                required: definition.required_capability,
            });
        }

        if request.target_count == 0 {
            return Err(OpsModelError::InvalidItemCount {
                operation: "bulk operation",
                count: request.target_count,
            });
        }

        if let Some(max_items) = definition.max_items {
            if request.target_count > max_items {
                return Err(OpsModelError::InvalidItemCount {
                    operation: "bulk operation",
                    count: request.target_count,
                });
            }
        }

        if definition.requires_idempotency_key && request.idempotency_key.is_none() {
            return Err(OpsModelError::InvalidBulkOperation {
                operation_id: definition.id.to_string(),
                reason: "idempotency key is required for retry-safe execution".to_string(),
            });
        }

        let queue_topology = self.jobs.describe_queue_topology();
        let queue = if request.scheduled_for.is_some() {
            queue_topology.scheduled_queue.clone()
        } else {
            queue_topology.work_queue.clone()
        };

        let job_id = JobId::new(request.execution_id.as_str().to_string())?;
        let job_name = JobName::new(format!("bulk.{}", definition.id.as_str()))?;
        let mut retry_policy = definition.retry_policy.clone();
        retry_policy =
            retry_policy.with_dead_letter_queue(queue_topology.dead_letter_queue.clone());

        let mut spec = JobSpec::new(
            job_id,
            job_name,
            queue,
            format!(
                "bulk `{}` on `{}` items",
                definition.kind, request.target_count
            ),
        )?;
        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }
        spec = spec.with_retry_policy(retry_policy);
        if let Some(key) = request.idempotency_key.clone() {
            spec = spec.with_idempotency_key(key);
        }

        let planned_job = self.jobs.plan_job(spec.clone(), request.requested_at)?;

        Ok(BulkOperationPlan {
            definition: definition.clone(),
            job: spec,
            planned_job,
            dry_run: request.dry_run,
            target_count: request.target_count,
            audit_message: format!(
                "bulk `{}` requested by `{}` for `{}` items",
                definition.id, request.requested_by, request.target_count
            ),
        })
    }
}

pub(crate) fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}
