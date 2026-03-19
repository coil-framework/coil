use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedReportExport {
    pub plan: ReportExportPlan,
    pub queued_job_id: JobId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedBulkOperation {
    pub plan: BulkOperationPlan,
    pub queued_job_id: JobId,
}

#[derive(Debug, Clone)]
pub struct OpsHost {
    planner: OpsPlanner,
    jobs: JobsHost,
}

impl OpsHost {
    pub(crate) fn new(planner: OpsPlanner, jobs: JobsHost) -> Self {
        Self { planner, jobs }
    }

    pub fn planner(&self) -> &OpsPlanner {
        &self.planner
    }

    pub fn jobs(&self) -> &JobsHost {
        &self.jobs
    }

    pub fn jobs_mut(&mut self) -> &mut JobsHost {
        &mut self.jobs
    }

    pub fn queue_report_export(
        &mut self,
        request: ReportExportRequest,
    ) -> Result<QueuedReportExport, RuntimeOpsError> {
        let requested_at = request.requested_at;
        let plan = self.planner.plan_report_export(request)?;
        let queued_job_id = self.jobs.enqueue_spec(plan.job.clone(), requested_at)?;

        Ok(QueuedReportExport {
            plan,
            queued_job_id,
        })
    }

    pub fn queue_bulk_operation(
        &mut self,
        request: BulkOperationRequest,
    ) -> Result<QueuedBulkOperation, RuntimeOpsError> {
        let requested_at = request.requested_at;
        let plan = self.planner.plan_bulk_operation(request)?;
        let queued_job_id = self.jobs.enqueue_spec(plan.job.clone(), requested_at)?;

        Ok(QueuedBulkOperation {
            plan,
            queued_job_id,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInvalidationPlan {
    pub trigger: SearchInvalidationTrigger,
    pub indexes: Vec<OpsSearchIndexContribution>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeSearchError {
    #[error(transparent)]
    Ops(#[from] RuntimeOpsError),
    #[error(transparent)]
    Model(#[from] OpsModelError),
    #[error(transparent)]
    Jobs(#[from] JobsModelError),
    #[error("search host requires at least one configured index contribution")]
    EmptyCatalog,
}

#[derive(Debug, Clone)]
pub struct SearchHost {
    catalog: SearchCatalog,
    ops: OpsHost,
}

impl SearchHost {
    pub(crate) fn new(catalog: SearchCatalog, ops: OpsHost) -> Self {
        Self { catalog, ops }
    }

    pub fn catalog(&self) -> &SearchCatalog {
        &self.catalog
    }

    pub fn visible_to(
        &self,
        capabilities: &[davenda_auth::Capability],
    ) -> Vec<&OpsSearchIndexContribution> {
        self.catalog.visible_to(capabilities)
    }

    pub fn indexes_for_trigger(
        &self,
        trigger: SearchInvalidationTrigger,
    ) -> Vec<&OpsSearchIndexContribution> {
        self.catalog
            .contributions
            .iter()
            .filter(|index| {
                index
                    .invalidation_rules
                    .iter()
                    .any(|rule| rule.trigger == trigger)
            })
            .collect()
    }

    pub fn invalidation_plan(&self, trigger: SearchInvalidationTrigger) -> SearchInvalidationPlan {
        SearchInvalidationPlan {
            trigger,
            indexes: self
                .indexes_for_trigger(trigger)
                .into_iter()
                .cloned()
                .collect(),
        }
    }

    pub fn scheduled_rebuilds(&self) -> Vec<&OpsSearchIndexContribution> {
        self.catalog
            .contributions
            .iter()
            .filter(|index| {
                matches!(
                    index.rebuild_strategy,
                    SearchRebuildStrategy::Scheduled { .. }
                )
            })
            .collect()
    }

    pub fn queue_full_reindex(
        &mut self,
        execution_id: BulkExecutionId,
        requested_by: impl Into<String>,
        requested_at: JobInstant,
        operator_capabilities: Vec<davenda_auth::Capability>,
        dry_run: bool,
    ) -> Result<QueuedBulkOperation, RuntimeSearchError> {
        if self.catalog.contributions.is_empty() {
            return Err(RuntimeSearchError::EmptyCatalog);
        }

        let idempotency_key =
            IdempotencyKey::new(format!("search.reindex:{}", execution_id.as_str()))?;
        let mut request = BulkOperationRequest::new(
            execution_id,
            BulkOperationId::new("bulk.search.reindex")?,
            requested_by,
            requested_at,
            self.catalog.contributions.len(),
        )?;

        for capability in operator_capabilities {
            request = request.with_capability(capability);
        }

        request = request
            .with_idempotency_key(idempotency_key)
            .dry_run(dry_run);
        Ok(self.ops.queue_bulk_operation(request)?)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeOpsError {
    #[error(transparent)]
    Ops(#[from] OpsModelError),
    #[error(transparent)]
    Jobs(#[from] RuntimeJobsError),
}
