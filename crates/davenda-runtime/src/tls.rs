use super::*;
use davenda_tls::{
    CertificateMaterial, ManualCertificateBundle, ManualImportTlsCertificateExecutor,
    TlsCertificateExecutor, TlsMaterialProtector,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeTlsError {
    #[error(transparent)]
    Tls(#[from] TlsModelError),
    #[error(transparent)]
    Data(#[from] davenda_data::DataModelError),
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
    control_plane: TlsControlPlaneRuntime,
    material_executor: ManualImportTlsCertificateExecutor,
}

impl TlsHost {
    pub(crate) fn new(
        customer_app: String,
        runtime: TlsRuntimeServices,
        _data_runtime: DataRuntimeServices,
        shared_backend_namespace: String,
    ) -> Result<Self, RuntimeTlsError> {
        let material_seed = {
            #[cfg(test)]
            {
                format!(
                    "test-tls-material:{}:{}",
                    customer_app, shared_backend_namespace
                )
            }

            #[cfg(not(test))]
            {
                _data_runtime.resolve_connection_url()?
            }
        };
        let material_protector = TlsMaterialProtector::from_seed(material_seed)?;
        #[cfg(test)]
        let control_plane =
            TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime.clone());
        #[cfg(not(test))]
        let control_plane = TlsControlPlaneRuntime::with_distributed_postgres_control_plane(
            runtime.clone(),
            &_data_runtime,
            format!("customer-app:{}:{}", customer_app, shared_backend_namespace),
        )?;
        let material_executor =
            ManualImportTlsCertificateExecutor::new(control_plane.clone(), material_protector);
        Ok(Self {
            customer_app,
            runtime,
            control_plane,
            material_executor,
        })
    }

    pub fn status(&self) -> TlsStatusSnapshot {
        let snapshot = self.control_plane.snapshot();
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
        Ok(self.control_plane.import_certificate(record)?)
    }

    pub fn import_manual_certificate(
        &mut self,
        bundle: ManualCertificateBundle,
    ) -> Result<(), RuntimeTlsError> {
        let bundle = self.runtime.planner().import_manual_certificate(bundle)?;
        Ok(self.material_executor.import_manual_certificate(bundle)?)
    }

    pub fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, RuntimeTlsError> {
        Ok(self
            .material_executor
            .certificate_material(certificate_id)?)
    }

    pub fn queue_renewal(
        &mut self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, RuntimeTlsError> {
        Ok(self.control_plane.queue_renewal(certificate_id, now)?)
    }

    pub fn begin_renewal(
        &mut self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, RuntimeTlsError> {
        Ok(self
            .control_plane
            .begin_renewal(certificate_id, replacement_certificate_id)?)
    }

    pub fn fail_renewal(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, RuntimeTlsError> {
        Ok(self.control_plane.fail_renewal(certificate_id)?)
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, RuntimeTlsError> {
        Ok(self
            .control_plane
            .activate_replacement(certificate_id, replacement)?)
    }

    pub fn control_plane(&self) -> &TlsControlPlaneRuntime {
        &self.control_plane
    }
}
