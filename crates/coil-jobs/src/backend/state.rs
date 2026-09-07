use super::*;

#[derive(Debug, Clone)]
pub(super) struct JobsBackendState {
    pub(super) runtime: JobsRuntime,
    pub(super) snapshot: JobsCoordinatorSnapshot,
}

impl JobsBackendState {
    pub(super) fn new(runtime: JobsRuntime) -> Self {
        Self {
            runtime,
            snapshot: JobsCoordinatorSnapshot::default(),
        }
    }

    pub(super) fn snapshot(&self) -> JobsCoordinatorSnapshot {
        self.snapshot.clone()
    }

    pub(super) fn enqueue(&mut self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        let planned = self.runtime.planner().plan_job(spec.clone(), now)?;
        let record = QueuedJobRecord {
            spec: JobSpec {
                queue: planned.queue,
                scheduled_for: planned.scheduled_for,
                retry_policy: planned.retry_policy,
                idempotency_key: planned.idempotency_key,
                ..spec
            },
            attempts: 0,
            enqueued_at: now,
        };

        if record.spec.scheduled_for.is_some() {
            self.snapshot.scheduled.push(record);
        } else {
            self.snapshot.ready.push(record);
        }

        Ok(())
    }

    pub(super) fn acquire_scheduler_leadership(
        &mut self,
        node_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        let node_id = crate::validation::require_non_empty("node_id", node_id.into())?;
        if let Some(current) = self.snapshot.leadership.as_ref() {
            if current.is_active(now) && current.node_id != node_id {
                return Err(JobsModelError::LeadershipConflict {
                    current_holder: current.node_id.clone(),
                    requested_holder: node_id,
                });
            }
        }

        let leadership = SchedulerLeadership {
            node_id,
            acquired_at: now,
            lease_until: now.checked_add(lease_ttl)?,
        };
        self.snapshot.leadership = Some(leadership.clone());
        Ok(leadership)
    }

    pub(super) fn promote_due_jobs(
        &mut self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        self.require_active_leadership(node_id, now)?;

        let mut promoted_ids = Vec::new();
        let mut remaining = Vec::new();
        for mut job in self.snapshot.scheduled.drain(..) {
            if job
                .spec
                .scheduled_for
                .is_some_and(|scheduled_for| scheduled_for <= now)
            {
                promoted_ids.push(job.spec.job_id.clone());
                job.spec.scheduled_for = None;
                self.snapshot.ready.push(job);
            } else {
                remaining.push(job);
            }
        }
        self.snapshot.scheduled = remaining;
        Ok(promoted_ids)
    }

    pub(super) fn lease_ready_jobs(
        &mut self,
        queue: &JobQueueName,
        worker_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        let worker_id = crate::validation::require_non_empty("worker_id", worker_id.into())?;
        self.runtime
            .topology
            .queue(queue)
            .ok_or_else(|| JobsModelError::UnknownQueue {
                queue: queue.to_string(),
            })?;

        let lease_until = now.checked_add(lease_ttl)?;
        let mut leased = Vec::new();
        let mut remaining = Vec::new();

        for job in self.snapshot.ready.drain(..) {
            if leased.len() < max_jobs && &job.spec.queue == queue {
                let lease = JobLease {
                    record: job,
                    worker_id: worker_id.clone(),
                    leased_at: now,
                    lease_until,
                };
                self.snapshot.in_flight.push(lease.clone());
                leased.push(lease);
            } else {
                remaining.push(job);
            }
        }

        self.snapshot.ready = remaining;
        Ok(leased)
    }

    pub(super) fn acknowledge_completed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        self.ensure_active_lease(lease, now)?;
        self.remove_in_flight(&lease.record.spec.job_id)?;
        Ok(())
    }

    pub(super) fn acknowledge_failed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: impl Into<String>,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        self.ensure_active_lease(lease, now)?;
        let error_message =
            crate::validation::require_non_empty("job_error_message", error_message.into())?;
        let mut record = self.remove_in_flight(&lease.record.spec.job_id)?;
        record.attempts += 1;

        if record.attempts < record.spec.retry_policy.max_attempts {
            let delay = record.spec.retry_policy.delay_for_attempt(record.attempts);
            let next_attempt_at = now.checked_add(delay)?;
            if delay.is_zero() {
                record.spec.scheduled_for = None;
                self.snapshot.ready.push(record.clone());
            } else {
                record.spec.scheduled_for = Some(next_attempt_at);
                self.snapshot.scheduled.push(record.clone());
            }

            Ok(JobFailureDisposition::Retried {
                job_id: record.spec.job_id,
                next_attempt_at,
                queue: record.spec.queue,
            })
        } else {
            let routed_to = record
                .spec
                .retry_policy
                .dead_letter_queue
                .clone()
                .or_else(|| {
                    self.runtime
                        .topology
                        .queue(&record.spec.queue)
                        .and_then(|queue| queue.dead_letter_queue.clone())
                });
            let outcome = DeadLetterOutcome::new(
                DeadLetterId::new(format!("dead-letter:{}", record.spec.job_id.as_str()))?,
                record.spec.job_id.clone(),
                record.spec.queue.clone(),
                record.spec.clone(),
                reason,
                record.attempts,
                error_message,
                routed_to,
            )?;
            self.snapshot.dead_letters.push(outcome.clone());
            Ok(JobFailureDisposition::DeadLettered(outcome))
        }
    }

    pub(super) fn retry_dead_letter(
        &mut self,
        dead_letter_id: &DeadLetterId,
        now: JobInstant,
    ) -> Result<QueuedJobRecord, JobsModelError> {
        let index = self
            .snapshot
            .dead_letters
            .iter()
            .position(|outcome| &outcome.dead_letter_id == dead_letter_id)
            .ok_or_else(|| JobsModelError::UnknownDeadLetter {
                dead_letter_id: dead_letter_id.to_string(),
            })?;
        let outcome = self.snapshot.dead_letters.remove(index);
        let mut spec = outcome.job_spec;
        spec.scheduled_for = None;
        let record = QueuedJobRecord {
            spec,
            attempts: 0,
            enqueued_at: now,
        };
        self.snapshot.ready.push(record.clone());
        Ok(record)
    }

    pub(super) fn cancel(
        &mut self,
        queue: &JobQueueName,
        job_id: &JobId,
    ) -> Result<bool, JobsModelError> {
        if let Some(index) = self
            .snapshot
            .ready
            .iter()
            .position(|j| &j.spec.queue == queue && &j.spec.job_id == job_id)
        {
            self.snapshot.ready.remove(index);
            return Ok(true);
        }

        if let Some(index) = self
            .snapshot
            .scheduled
            .iter()
            .position(|j| &j.spec.queue == queue && &j.spec.job_id == job_id)
        {
            self.snapshot.scheduled.remove(index);
            return Ok(true);
        }

        if let Some(index) = self
            .snapshot
            .in_flight
            .iter()
            .position(|l| &l.record.spec.queue == queue && &l.record.spec.job_id == job_id)
        {
            self.snapshot.in_flight.remove(index);
            return Ok(true);
        }

        Ok(false)
    }

    fn require_active_leadership(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        match self.snapshot.leadership.as_ref() {
            Some(leadership) if leadership.node_id == node_id && leadership.is_active(now) => {
                Ok(())
            }
            Some(leadership) if leadership.node_id == node_id => {
                Err(JobsModelError::SchedulerLeadershipExpired {
                    node_id: node_id.to_string(),
                    lease_until: leadership.lease_until,
                    now,
                })
            }
            Some(_) | None => Err(JobsModelError::MissingSchedulerLeadership {
                node_id: node_id.to_string(),
            }),
        }
    }

    fn ensure_active_lease(&self, lease: &JobLease, now: JobInstant) -> Result<(), JobsModelError> {
        if lease.lease_until <= now {
            return Err(JobsModelError::LeaseExpired {
                job_id: lease.record.spec.job_id.to_string(),
                lease_until: lease.lease_until,
                now,
            });
        }

        self.snapshot
            .in_flight
            .iter()
            .find(|current| current.record.spec.job_id == lease.record.spec.job_id)
            .ok_or_else(|| JobsModelError::UnknownInFlightJob {
                job_id: lease.record.spec.job_id.to_string(),
            })?;

        Ok(())
    }

    fn remove_in_flight(&mut self, job_id: &JobId) -> Result<QueuedJobRecord, JobsModelError> {
        let index = self
            .snapshot
            .in_flight
            .iter()
            .position(|lease| &lease.record.spec.job_id == job_id)
            .ok_or_else(|| JobsModelError::UnknownInFlightJob {
                job_id: job_id.to_string(),
            })?;
        Ok(self.snapshot.in_flight.remove(index).record)
    }
}
