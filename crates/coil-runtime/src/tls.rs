use super::*;
use crate::server::SecretResolutionError;
use coil_tls::{
    AcmeTlsCertificateExecutor, CertificateMaterial, ChallengeValidation,
    CloudflareTlsCertificateExecutor, HostnameBinding, ManualCertificateBundle,
    ManualImportTlsCertificateExecutor, TlsCertificateExecutor, TlsMaterialProtector,
};
use std::sync::Arc;

#[cfg(not(test))]
const TLS_MATERIAL_KEY_ENV: &str = "COIL_TLS_MATERIAL_KEY";
const TLS_PREVIOUS_MATERIAL_KEYS_ENV: &str = "COIL_TLS_PREVIOUS_MATERIAL_KEYS";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeTlsError {
    #[error(transparent)]
    Tls(#[from] TlsModelError),
    #[error(transparent)]
    Data(#[from] coil_data::DataModelError),
    #[error(transparent)]
    Secret(#[from] SecretResolutionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsStatusSnapshot {
    pub customer_app: String,
    pub mode: coil_config::TlsMode,
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
    certificate_executor: Arc<dyn TlsCertificateExecutor>,
}

impl TlsHost {
    fn build_executor(
        customer_app: &str,
        shared_backend_namespace: &str,
        runtime: &TlsRuntimeServices,
        control_plane: TlsControlPlaneRuntime,
        account_secret: Option<String>,
        material_protector: TlsMaterialProtector,
    ) -> Result<Self, RuntimeTlsError> {
        let certificate_executor: Arc<dyn TlsCertificateExecutor> = match runtime.provider {
            Some(coil_tls::CertificateProviderKind::Acme) => {
                Arc::new(AcmeTlsCertificateExecutor::new(
                    control_plane.clone(),
                    material_protector,
                    account_secret.clone(),
                ))
            }
            Some(coil_tls::CertificateProviderKind::CloudflareDns)
            | Some(coil_tls::CertificateProviderKind::CloudflareOriginCa) => {
                Arc::new(CloudflareTlsCertificateExecutor::new(
                    runtime
                        .provider
                        .expect("cloudflare provider is selected when creating executor"),
                    control_plane.clone(),
                    material_protector,
                    account_secret.clone(),
                ))
            }
            Some(coil_tls::CertificateProviderKind::ManualImport) | None => Arc::new(
                ManualImportTlsCertificateExecutor::new(control_plane.clone(), material_protector),
            ),
        };
        Ok(Self {
            customer_app: customer_app.to_string(),
            runtime: runtime.clone(),
            control_plane,
            certificate_executor,
        })
    }

    pub(crate) fn new(
        customer_app: String,
        runtime: TlsRuntimeServices,
        _data_runtime: DataRuntimeServices,
        shared_backend_namespace: String,
        account_secret: Option<String>,
    ) -> Result<Self, RuntimeTlsError> {
        #[cfg(test)]
        let material_protector = TlsMaterialProtector::from_seed(format!(
            "test-tls-material:{}:{}",
            customer_app, shared_backend_namespace
        ))?;
        #[cfg(not(test))]
        let material_protector = runtime_material_protector()?;
        #[cfg(test)]
        let control_plane =
            TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime.clone());
        #[cfg(not(test))]
        let control_plane = TlsControlPlaneRuntime::with_distributed_postgres_control_plane(
            runtime.clone(),
            &_data_runtime,
            format!("customer-app:{}:{}", customer_app, shared_backend_namespace),
        )?;
        Self::build_executor(
            &customer_app,
            &shared_backend_namespace,
            &runtime,
            control_plane,
            account_secret,
            material_protector,
        )
    }

    pub(crate) fn new_for_validation(
        customer_app: String,
        runtime: TlsRuntimeServices,
        shared_backend_namespace: String,
        account_secret: Option<String>,
    ) -> Result<Self, RuntimeTlsError> {
        let material_protector = TlsMaterialProtector::from_seed(format!(
            "tls-validation:{}:{}",
            customer_app, shared_backend_namespace
        ))?;
        let control_plane =
            TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime.clone());
        Self::build_executor(
            &customer_app,
            &shared_backend_namespace,
            &runtime,
            control_plane,
            account_secret,
            material_protector,
        )
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

    pub fn validate_challenge_for_bindings(
        &self,
        bindings: Vec<HostnameBinding>,
    ) -> Result<ChallengeValidation, RuntimeTlsError> {
        let plan = self.issue_for_bindings(bindings)?;
        Ok(self.certificate_executor.validate_issuance_plan(&plan)?)
    }

    pub fn import_certificate(&mut self, record: CertificateRecord) -> Result<(), RuntimeTlsError> {
        Ok(self.control_plane.import_certificate(record)?)
    }

    pub fn import_manual_certificate(
        &mut self,
        bundle: ManualCertificateBundle,
    ) -> Result<(), RuntimeTlsError> {
        let bundle = self.runtime.planner().import_manual_certificate(bundle)?;
        Ok(self
            .certificate_executor
            .import_manual_certificate(bundle)?)
    }

    pub fn certificate_material(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateMaterial, RuntimeTlsError> {
        Ok(self
            .certificate_executor
            .certificate_material(certificate_id)?)
    }

    pub fn issue_certificate(
        &mut self,
        bindings: Vec<HostnameBinding>,
        certificate_id: CertificateId,
        now: TlsInstant,
    ) -> Result<CertificateRecord, RuntimeTlsError> {
        let issuance = self.issue_for_bindings(bindings)?;
        let record = self
            .certificate_executor
            .issue_certificate(&issuance, certificate_id, now)?;
        self.control_plane.import_certificate(record.clone())?;
        Ok(record)
    }

    pub fn renew_certificate(
        &mut self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
        now: TlsInstant,
    ) -> Result<CertificateRecord, RuntimeTlsError> {
        let renewal_plan = self.queue_renewal(certificate_id, now)?;
        let _ticket = self.begin_renewal(certificate_id, replacement_certificate_id.clone())?;
        let record = match self.certificate_executor.renew_certificate(
            &renewal_plan,
            certificate_id.clone(),
            replacement_certificate_id,
            now,
        ) {
            Ok(record) => record,
            Err(error) => {
                let _ = self.fail_renewal(certificate_id);
                return Err(error.into());
            }
        };
        self.control_plane
            .activate_replacement(certificate_id, record.clone())?;
        Ok(record)
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

#[cfg(not(test))]
fn runtime_material_protector() -> Result<TlsMaterialProtector, RuntimeTlsError> {
    let active_key = std::env::var(TLS_MATERIAL_KEY_ENV).map_err(|_| {
        TlsModelError::InvalidConfiguration {
            field: "tls.material_encryption_key",
            reason: format!(
                "set `{TLS_MATERIAL_KEY_ENV}` so certificate material is encrypted with a dedicated TLS secret"
            ),
        }
    })?;
    let active_key = active_key.trim().to_string();
    if active_key.is_empty() {
        return Err(TlsModelError::InvalidConfiguration {
            field: "tls.material_encryption_key",
            reason: format!(
                "`{TLS_MATERIAL_KEY_ENV}` must not be empty when TLS certificate material is stored by the platform"
            ),
        }
        .into());
    }
    let previous_keys =
        parse_previous_material_keys(std::env::var(TLS_PREVIOUS_MATERIAL_KEYS_ENV).ok())?;
    Ok(TlsMaterialProtector::from_seed_ring(
        active_key,
        previous_keys,
    )?)
}

fn parse_previous_material_keys(value: Option<String>) -> Result<Vec<String>, RuntimeTlsError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };

    let keys = value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    if keys.is_empty() {
        return Err(TlsModelError::InvalidConfiguration {
            field: "tls.previous_material_encryption_keys",
            reason: format!(
                "`{TLS_PREVIOUS_MATERIAL_KEYS_ENV}` was set but did not contain any usable key material"
            ),
        }
        .into());
    }

    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeBuilder;
    use coil_auth::DefaultAuthModelPackage;
    use coil_config::{AcmeChallenge, PlatformConfig, SecretRef, TlsMode};
    use coil_tls::{
        CertificateId, CertificateProviderKind, CustomerAppId, Hostname, HostnameBinding,
    };

    const TLS_RUNTIME_TEST_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "coil_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "coil_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/tmp/coil-runtime-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "coil-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms-pages", "admin-shell"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#;

    fn tls_runtime_test_config() -> PlatformConfig {
        PlatformConfig::from_toml_str(TLS_RUNTIME_TEST_CONFIG).unwrap()
    }

    fn binding(hostname: &str) -> HostnameBinding {
        HostnameBinding::new(
            Hostname::new(hostname).unwrap(),
            CustomerAppId::new("showcase-events").unwrap(),
        )
    }

    #[test]
    fn previous_tls_material_key_list_accepts_comma_and_newline_delimiters() {
        let keys = parse_previous_material_keys(Some("old-a,\nold-b\nold-c".to_string())).unwrap();

        assert_eq!(keys, vec!["old-a", "old-b", "old-c"]);
    }

    #[test]
    fn previous_tls_material_key_list_rejects_empty_configured_values() {
        let error = parse_previous_material_keys(Some(" , \n ".to_string())).unwrap_err();

        assert_eq!(
            error,
            RuntimeTlsError::Tls(TlsModelError::InvalidConfiguration {
                field: "tls.previous_material_encryption_keys",
                reason: format!(
                    "`{TLS_PREVIOUS_MATERIAL_KEYS_ENV}` was set but did not contain any usable key material"
                ),
            })
        );
    }

    #[test]
    fn tls_host_uses_real_acme_executor_in_tests() {
        let mut config = tls_runtime_test_config();
        config.tls.mode = TlsMode::Acme;
        config.tls.challenge = Some(AcmeChallenge::TlsAlpn01);
        config.tls.provider = None;
        config.tls.account_secret = Some(SecretRef::SecretManager {
            provider: "vault".to_string(),
            key: "tls/acme".to_string(),
        });
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let resolver = crate::server::StaticSecretResolver::new()
            .with_secret(
                SecretRef::SecretManager {
                    provider: "vault".to_string(),
                    key: "tls/acme".to_string(),
                },
                r#"{"tls_alpn_bind_address":"not-a-socket-address"}"#,
            )
            .unwrap();
        let mut host = plan.tls_host_with_secret_resolver(&resolver).unwrap();

        let error = host
            .issue_certificate(
                vec![binding("www.example.com")],
                CertificateId::new("cert-real-acme-runtime").unwrap(),
                TlsInstant::from_unix_seconds(1_700_000_000),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeTlsError::Tls(TlsModelError::ProviderRequestFailed {
                provider,
                operation,
                ..
            }) if provider == CertificateProviderKind::Acme.to_string()
                && operation == "parse_tls_alpn_bind_address"
        ));
    }
}
