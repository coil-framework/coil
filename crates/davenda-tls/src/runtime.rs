use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuancePlan {
    pub edge_mode: EdgeMode,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub state_store: CertificateStateStore,
    pub bindings: Vec<HostnameBinding>,
    pub shared_across_nodes: bool,
    pub requires_hot_reload: bool,
    pub account_secret: Option<String>,
    pub cloudflare_mode: Option<CloudflareEncryptionMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenewalPlan {
    pub certificate_id: CertificateId,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub renew_after: TlsInstant,
    pub must_complete_by: TlsInstant,
    pub retry_interval: Duration,
    pub keep_serving_current_certificate: bool,
    pub requires_hot_reload: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeTicket {
    pub certificate_id: CertificateId,
    pub replacement_certificate_id: Option<CertificateId>,
    pub provider: CertificateProviderKind,
    pub challenge: Option<ChallengeStrategy>,
    pub bindings: Vec<HostnameBinding>,
    pub account_secret_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotReloadEvent {
    pub certificate_id: CertificateId,
    pub bindings: Vec<HostnameBinding>,
    pub reloaded_without_restart: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CertificateInventory {
    certificates: Vec<CertificateRecord>,
}

impl CertificateInventory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn certificates(&self) -> &[CertificateRecord] {
        &self.certificates
    }

    pub fn active_for_hostname(&self, hostname: &Hostname) -> Option<&CertificateRecord> {
        self.certificates.iter().find(|record| {
            record
                .bindings
                .iter()
                .any(|binding| &binding.hostname == hostname)
                && matches!(
                    record.status,
                    CertificateStatus::Active
                        | CertificateStatus::RenewalDue
                        | CertificateStatus::Renewing
                )
        })
    }

    pub fn record(&self, certificate_id: &CertificateId) -> Option<&CertificateRecord> {
        self.certificates
            .iter()
            .find(|record| &record.id == certificate_id)
    }

    pub fn record_mut(&mut self, certificate_id: &CertificateId) -> Option<&mut CertificateRecord> {
        self.certificates
            .iter_mut()
            .find(|record| &record.id == certificate_id)
    }

    pub fn insert(&mut self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.ensure_unique_bindings(&record, None)?;
        self.certificates.push(record);
        Ok(())
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<(), TlsModelError> {
        let original =
            self.record(certificate_id)
                .ok_or_else(|| TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                })?;
        if original.replacing_certificate.as_ref() != Some(&replacement.id) {
            return Err(TlsModelError::MissingReplacementCertificate {
                certificate_id: certificate_id.to_string(),
            });
        }

        self.ensure_unique_bindings(&replacement, Some(certificate_id))?;
        let original =
            self.record_mut(certificate_id)
                .ok_or_else(|| TlsModelError::UnknownCertificate {
                    certificate_id: certificate_id.to_string(),
                })?;
        original.status = CertificateStatus::Superseded;
        original.replacing_certificate = None;

        self.certificates.push(replacement);
        Ok(())
    }

    fn ensure_unique_bindings(
        &self,
        candidate: &CertificateRecord,
        allowing_replaced_certificate: Option<&CertificateId>,
    ) -> Result<(), TlsModelError> {
        for binding in &candidate.bindings {
            if let Some(existing) = self.active_for_hostname(&binding.hostname) {
                let allowed = allowing_replaced_certificate
                    .is_some_and(|certificate_id| &existing.id == certificate_id);
                if !allowed {
                    return Err(TlsModelError::DuplicateHostnameBinding {
                        hostname: binding.hostname.to_string(),
                        certificate_id: existing.id.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsAutomationRuntime {
    runtime: TlsRuntime,
    inventory: CertificateInventory,
    renewal_queue: Vec<RenewalPlan>,
    pending_challenges: Vec<ChallengeTicket>,
    hot_reload_events: Vec<HotReloadEvent>,
}

impl TlsAutomationRuntime {
    pub fn new(runtime: TlsRuntime) -> Self {
        Self {
            runtime,
            inventory: CertificateInventory::new(),
            renewal_queue: Vec::new(),
            pending_challenges: Vec::new(),
            hot_reload_events: Vec::new(),
        }
    }

    pub fn inventory(&self) -> &CertificateInventory {
        &self.inventory
    }

    pub fn renewal_queue(&self) -> &[RenewalPlan] {
        &self.renewal_queue
    }

    pub fn pending_challenges(&self) -> &[ChallengeTicket] {
        &self.pending_challenges
    }

    pub fn hot_reload_events(&self) -> &[HotReloadEvent] {
        &self.hot_reload_events
    }

    pub fn import_certificate(&mut self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.inventory.insert(record)
    }

    pub fn queue_renewal(
        &mut self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        let record = self
            .inventory
            .record(certificate_id)
            .cloned()
            .ok_or_else(|| TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            })?;
        let plan = self.runtime.planner().renewal_plan(&record, now)?;
        if plan.renew_after > now {
            return Err(TlsModelError::RenewalNotDue {
                certificate_id: certificate_id.to_string(),
                renew_after: plan.renew_after,
                now,
            });
        }

        if let Some(existing) = self
            .renewal_queue
            .iter()
            .find(|plan| plan.certificate_id == *certificate_id)
        {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: existing.certificate_id.to_string(),
            });
        }

        if let Some(record) = self.inventory.record_mut(certificate_id) {
            record.status = CertificateStatus::RenewalDue;
        }
        self.renewal_queue.push(plan.clone());
        Ok(plan)
    }

    pub fn begin_renewal(
        &mut self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        let record = self.inventory.record_mut(certificate_id).ok_or_else(|| {
            TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            }
        })?;
        if record.replacing_certificate.is_some() {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: certificate_id.to_string(),
            });
        }

        record.status = CertificateStatus::Renewing;
        record.replacing_certificate = Some(replacement_certificate_id.clone());

        let ticket = ChallengeTicket {
            certificate_id: certificate_id.clone(),
            replacement_certificate_id: Some(replacement_certificate_id),
            provider: record.provider,
            challenge: self.runtime.challenge,
            bindings: record.bindings.clone(),
            account_secret_ref: self.runtime.account_secret_ref.clone(),
        };
        self.pending_challenges.push(ticket.clone());
        Ok(ticket)
    }

    pub fn fail_renewal(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        let record = self.inventory.record_mut(certificate_id).ok_or_else(|| {
            TlsModelError::UnknownCertificate {
                certificate_id: certificate_id.to_string(),
            }
        })?;
        record.status = CertificateStatus::RenewalDue;
        record.replacing_certificate = None;
        self.pending_challenges
            .retain(|ticket| &ticket.certificate_id != certificate_id);
        self.renewal_queue
            .retain(|plan| &plan.certificate_id != certificate_id);
        Ok(record.clone())
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        mut replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        replacement.status = CertificateStatus::Active;
        replacement.replacing_certificate = None;

        self.inventory
            .activate_replacement(certificate_id, replacement.clone())?;
        self.pending_challenges
            .retain(|ticket| &ticket.certificate_id != certificate_id);
        self.renewal_queue
            .retain(|plan| &plan.certificate_id != certificate_id);

        let event = HotReloadEvent {
            certificate_id: replacement.id.clone(),
            bindings: replacement.bindings.clone(),
            reloaded_without_restart: self.runtime.hot_reload_supported,
        };
        self.hot_reload_events.push(event.clone());
        Ok(event)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsRuntime {
    pub mode: TlsMode,
    pub edge_mode: EdgeMode,
    pub provider: Option<CertificateProviderKind>,
    pub challenge: Option<ChallengeStrategy>,
    pub state_store: CertificateStateStore,
    pub shared_across_nodes: bool,
    pub requires_trusted_termination_metadata: bool,
    pub hot_reload_supported: bool,
    pub cloudflare_mode: Option<CloudflareEncryptionMode>,
    pub account_secret_ref: Option<String>,
}

impl TlsRuntime {
    pub fn from_config(config: &TlsConfig) -> Self {
        let account_secret_ref = config.account_secret.as_ref().map(SecretRef::redacted);

        match config.mode {
            TlsMode::External => Self {
                mode: config.mode,
                edge_mode: EdgeMode::ExternalTermination,
                provider: None,
                challenge: None,
                state_store: CertificateStateStore::ExternalTermination,
                shared_across_nodes: true,
                requires_trusted_termination_metadata: true,
                hot_reload_supported: false,
                cloudflare_mode: None,
                account_secret_ref,
            },
            TlsMode::Acme => {
                let provider = match config.provider {
                    Some(TlsProvider::CloudflareDns) => CertificateProviderKind::CloudflareDns,
                    _ => CertificateProviderKind::Acme,
                };

                Self {
                    mode: config.mode,
                    edge_mode: EdgeMode::DirectTermination,
                    provider: Some(provider),
                    challenge: config.challenge.map(ChallengeStrategy::from),
                    state_store: CertificateStateStore::SharedSecrets,
                    shared_across_nodes: true,
                    requires_trusted_termination_metadata: false,
                    hot_reload_supported: true,
                    cloudflare_mode: None,
                    account_secret_ref,
                }
            }
            TlsMode::CloudflareOrigin => Self {
                mode: config.mode,
                edge_mode: EdgeMode::CloudflareOriginOnly,
                provider: Some(CertificateProviderKind::CloudflareOriginCa),
                challenge: None,
                state_store: CertificateStateStore::SharedSecrets,
                shared_across_nodes: true,
                requires_trusted_termination_metadata: false,
                hot_reload_supported: true,
                cloudflare_mode: Some(CloudflareEncryptionMode::FullStrict),
                account_secret_ref,
            },
            TlsMode::Manual => Self {
                mode: config.mode,
                edge_mode: EdgeMode::DirectTermination,
                provider: Some(CertificateProviderKind::ManualImport),
                challenge: None,
                state_store: CertificateStateStore::OperatorManaged,
                shared_across_nodes: true,
                requires_trusted_termination_metadata: false,
                hot_reload_supported: true,
                cloudflare_mode: None,
                account_secret_ref,
            },
        }
    }

    pub fn planner(&self) -> TlsPlanner {
        TlsPlanner {
            runtime: self.clone(),
        }
    }

    pub fn automation(&self) -> TlsAutomationRuntime {
        TlsAutomationRuntime::new(self.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsPlanner {
    runtime: TlsRuntime,
}

impl TlsPlanner {
    pub fn issue_for_bindings(
        &self,
        bindings: Vec<HostnameBinding>,
    ) -> Result<IssuancePlan, TlsModelError> {
        if self.runtime.mode == TlsMode::External {
            return Err(TlsModelError::ExternalTerminationDoesNotIssue);
        }

        if self.runtime.mode == TlsMode::Manual {
            return Err(TlsModelError::ManualModeRequiresImportedCertificate);
        }

        if bindings.iter().any(HostnameBinding::is_wildcard)
            && self
                .runtime
                .challenge
                .is_none_or(|challenge| !challenge.supports_wildcards())
        {
            return Err(TlsModelError::WildcardRequiresDns01);
        }

        Ok(IssuancePlan {
            edge_mode: self.runtime.edge_mode,
            provider: self.runtime.provider.unwrap(),
            challenge: self.runtime.challenge,
            state_store: self.runtime.state_store,
            bindings,
            shared_across_nodes: self.runtime.shared_across_nodes,
            requires_hot_reload: self.runtime.hot_reload_supported,
            account_secret: self.runtime.account_secret_ref.clone(),
            cloudflare_mode: self.runtime.cloudflare_mode,
        })
    }

    pub fn import_manual_certificate(
        &self,
        record: CertificateRecord,
    ) -> Result<CertificateRecord, TlsModelError> {
        if self.runtime.mode != TlsMode::Manual {
            return Err(TlsModelError::ManualModeRequiresImportedCertificate);
        }

        Ok(record)
    }

    pub fn renewal_plan(
        &self,
        record: &CertificateRecord,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        if !matches!(
            record.status,
            CertificateStatus::Active | CertificateStatus::RenewalDue
        ) {
            return Err(TlsModelError::CertificateNotActive {
                certificate_id: record.id.to_string(),
            });
        }

        if record.replacing_certificate.is_some() {
            return Err(TlsModelError::RenewalAlreadyInProgress {
                certificate_id: record.id.to_string(),
            });
        }

        let window = record.renewal_window();
        Ok(RenewalPlan {
            certificate_id: record.id.clone(),
            provider: record.provider,
            challenge: self.runtime.challenge,
            renew_after: if now > window.starts_at {
                now
            } else {
                window.starts_at
            },
            must_complete_by: window.must_complete_by,
            retry_interval: window.retry_interval,
            keep_serving_current_certificate: true,
            requires_hot_reload: self.runtime.hot_reload_supported,
        })
    }
}
