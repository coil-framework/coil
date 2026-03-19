use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeTlsError {
    #[error(transparent)]
    Tls(#[from] TlsModelError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsStatusSnapshot {
    pub customer_app: String,
    pub mode: davenda_config::TlsMode,
    pub edge_mode: EdgeMode,
    pub provider: Option<CertificateProviderKind>,
    pub inventory: CertificateInventory,
    pub queued_renewals: Vec<RenewalPlan>,
    pub pending_challenges: Vec<ChallengeTicket>,
    pub hot_reload_events: Vec<HotReloadEvent>,
}

#[derive(Debug, Clone)]
pub struct TlsHost {
    pub customer_app: String,
    pub runtime: TlsRuntimeServices,
    automation: TlsAutomationRuntime,
}

impl TlsHost {
    pub(crate) fn new(
        customer_app: String,
        runtime: TlsRuntimeServices,
        _data_runtime: DataRuntimeServices,
        _shared_backend_namespace: String,
    ) -> Result<Self, RuntimeTlsError> {
        #[cfg(test)]
        let automation = TlsAutomationRuntime::in_memory_for_tests(runtime.clone());
        #[cfg(not(test))]
        let automation = TlsAutomationRuntime::with_postgres_shared_backend(
            runtime.clone(),
            &_data_runtime,
            format!("customer-app:{}:{}", customer_app, _shared_backend_namespace),
        )?;
        Ok(Self {
            customer_app,
            runtime,
            automation,
        })
    }

    pub fn status(&self) -> TlsStatusSnapshot {
        let snapshot = self.automation.snapshot();
        TlsStatusSnapshot {
            customer_app: self.customer_app.clone(),
            mode: self.runtime.mode,
            edge_mode: self.runtime.edge_mode,
            provider: self.runtime.provider,
            inventory: snapshot.inventory,
            queued_renewals: snapshot.renewal_queue,
            pending_challenges: snapshot.pending_challenges,
            hot_reload_events: snapshot.hot_reload_events,
        }
    }

    pub fn issue_for_bindings(
        &self,
        bindings: Vec<HostnameBinding>,
    ) -> Result<IssuancePlan, RuntimeTlsError> {
        Ok(self.runtime.planner().issue_for_bindings(bindings)?)
    }

    pub fn import_certificate(&mut self, record: CertificateRecord) -> Result<(), RuntimeTlsError> {
        Ok(self.automation.import_certificate(record)?)
    }

    pub fn queue_renewal(
        &mut self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, RuntimeTlsError> {
        Ok(self.automation.queue_renewal(certificate_id, now)?)
    }

    pub fn begin_renewal(
        &mut self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, RuntimeTlsError> {
        Ok(self
            .automation
            .begin_renewal(certificate_id, replacement_certificate_id)?)
    }

    pub fn fail_renewal(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, RuntimeTlsError> {
        Ok(self.automation.fail_renewal(certificate_id)?)
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, RuntimeTlsError> {
        Ok(self
            .automation
            .activate_replacement(certificate_id, replacement)?)
    }

    pub fn automation(&self) -> &TlsAutomationRuntime {
        &self.automation
    }
}
