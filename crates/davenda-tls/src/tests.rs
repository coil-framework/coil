use std::path::PathBuf;

use super::*;
use davenda_config::{
    AcmeChallenge, DatabaseConfig, DatabaseDriver, SecretRef, TlsConfig, TlsMode, TlsProvider,
};
use davenda_data::DataRuntime;

fn acme_config(challenge: AcmeChallenge, provider: Option<TlsProvider>) -> TlsConfig {
    TlsConfig {
        mode: TlsMode::Acme,
        challenge: Some(challenge),
        provider,
        account_secret: Some(SecretRef::Env {
            var: "TLS_ACCOUNT_KEY".to_string(),
        }),
    }
}

#[test]
fn acme_dns_runtime_prefers_shared_state_and_hot_reload() {
    let runtime = TlsRuntime::from_config(&acme_config(
        AcmeChallenge::Dns01,
        Some(TlsProvider::CloudflareDns),
    ));
    let plan = runtime
        .planner()
        .issue_for_bindings(vec![
            HostnameBinding::new(
                Hostname::new("*.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            ),
            HostnameBinding::new(
                Hostname::new("www.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            ),
        ])
        .unwrap();

    assert_eq!(
        runtime.provider,
        Some(CertificateProviderKind::CloudflareDns)
    );
    assert_eq!(plan.challenge, Some(ChallengeStrategy::Dns01));
    assert_eq!(plan.state_store, CertificateStateStore::SharedSecrets);
    assert!(plan.shared_across_nodes);
    assert!(plan.requires_hot_reload);
    assert_eq!(plan.account_secret.as_deref(), Some("env:TLS_ACCOUNT_KEY"));
}

#[test]
fn cloudflare_origin_runtime_forces_full_strict_origin_behavior() {
    let runtime = TlsRuntime::from_config(&TlsConfig {
        mode: TlsMode::CloudflareOrigin,
        challenge: None,
        provider: Some(TlsProvider::CloudflareOriginCa),
        account_secret: None,
    });
    let plan = runtime
        .planner()
        .issue_for_bindings(vec![HostnameBinding::new(
            Hostname::new("origin.example.com").unwrap(),
            CustomerAppId::new("storefront").unwrap(),
        )])
        .unwrap();

    assert_eq!(runtime.edge_mode, EdgeMode::CloudflareOriginOnly);
    assert_eq!(plan.provider, CertificateProviderKind::CloudflareOriginCa);
    assert_eq!(
        plan.cloudflare_mode,
        Some(CloudflareEncryptionMode::FullStrict)
    );
    assert!(plan.requires_hot_reload);
}

#[test]
fn external_termination_uses_trusted_metadata_and_does_not_issue_certificates() {
    let runtime = TlsRuntime::from_config(&TlsConfig {
        mode: TlsMode::External,
        challenge: None,
        provider: None,
        account_secret: None,
    });

    assert!(runtime.requires_trusted_termination_metadata);
    assert_eq!(
        runtime.state_store,
        CertificateStateStore::ExternalTermination
    );
    assert_eq!(
        runtime
            .planner()
            .issue_for_bindings(vec![HostnameBinding::new(
                Hostname::new("www.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            )])
            .unwrap_err(),
        TlsModelError::ExternalTerminationDoesNotIssue
    );
}

#[test]
fn renewal_keeps_current_certificate_live_until_replacement_succeeds() {
    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
    let record = CertificateRecord::new(
        CertificateId::new("cert-1").unwrap(),
        CertificateProviderKind::Acme,
        CertificateStatus::Active,
        CertificateFingerprint::new("sha256:abc123").unwrap(),
        TlsInstant::from_unix_seconds(1_000),
        TlsInstant::from_unix_seconds(4_000_000),
        SecretMaterialRef::new("secrets/tls/cert-1").unwrap(),
        CertificateStateStore::SharedSecrets,
    )
    .with_binding(HostnameBinding::new(
        Hostname::new("www.example.com").unwrap(),
        CustomerAppId::new("storefront").unwrap(),
    ));

    let renewal = runtime
        .planner()
        .renewal_plan(&record, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();
    assert_eq!(renewal.certificate_id.as_str(), "cert-1");
    assert!(renewal.keep_serving_current_certificate);
    assert!(renewal.requires_hot_reload);
    assert_eq!(renewal.challenge, Some(ChallengeStrategy::Dns01));
}

#[test]
fn wildcard_bindings_are_rejected_without_dns_validation() {
    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Http01, None));
    let error = runtime
        .planner()
        .issue_for_bindings(vec![HostnameBinding::new(
            Hostname::new("*.example.com").unwrap(),
            CustomerAppId::new("storefront").unwrap(),
        )])
        .unwrap_err();

    assert_eq!(error, TlsModelError::WildcardRequiresDns01);
}

#[test]
fn inventory_rejects_duplicate_active_hostname_bindings() {
    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
    let control_plane = TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime);
    let binding = HostnameBinding::new(
        Hostname::new("www.example.com").unwrap(),
        CustomerAppId::new("storefront").unwrap(),
    );

    control_plane
        .import_certificate(
            CertificateRecord::new(
                CertificateId::new("cert-1").unwrap(),
                CertificateProviderKind::Acme,
                CertificateStatus::Active,
                CertificateFingerprint::new("sha256:abc123").unwrap(),
                TlsInstant::from_unix_seconds(1_000),
                TlsInstant::from_unix_seconds(4_000_000),
                SecretMaterialRef::new("secrets/tls/cert-1").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(binding.clone()),
        )
        .unwrap();

    let error = control_plane
        .import_certificate(
            CertificateRecord::new(
                CertificateId::new("cert-2").unwrap(),
                CertificateProviderKind::Acme,
                CertificateStatus::Active,
                CertificateFingerprint::new("sha256:def456").unwrap(),
                TlsInstant::from_unix_seconds(2_000),
                TlsInstant::from_unix_seconds(4_000_000),
                SecretMaterialRef::new("secrets/tls/cert-2").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(binding),
        )
        .unwrap_err();

    assert_eq!(
        error,
        TlsModelError::DuplicateHostnameBinding {
            hostname: "www.example.com".to_string(),
            certificate_id: "cert-1".to_string(),
        }
    );
}

#[test]
fn renewal_failure_keeps_current_certificate_bound() {
    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
    let control_plane = TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime);
    let certificate_id = CertificateId::new("cert-active").unwrap();
    let binding = HostnameBinding::new(
        Hostname::new("www.example.com").unwrap(),
        CustomerAppId::new("storefront").unwrap(),
    );
    control_plane
        .import_certificate(
            CertificateRecord::new(
                certificate_id.clone(),
                CertificateProviderKind::Acme,
                CertificateStatus::Active,
                CertificateFingerprint::new("sha256:active").unwrap(),
                TlsInstant::from_unix_seconds(1_000),
                TlsInstant::from_unix_seconds(4_000_000),
                SecretMaterialRef::new("secrets/tls/cert-active").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(binding.clone()),
        )
        .unwrap();

    let queued = control_plane
        .queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();
    assert_eq!(queued.certificate_id, certificate_id);

    let challenge = control_plane
        .begin_renewal(
            &certificate_id,
            CertificateId::new("cert-replacement").unwrap(),
        )
        .unwrap();
    assert_eq!(
        challenge.replacement_certificate_id,
        Some(CertificateId::new("cert-replacement").unwrap())
    );

    let reverted = control_plane.fail_renewal(&certificate_id).unwrap();
    assert_eq!(reverted.status, CertificateStatus::RenewalDue);
    assert!(reverted.replacing_certificate.is_none());
    assert_eq!(
        control_plane
            .inventory()
            .active_for_hostname(&Hostname::new("www.example.com").unwrap())
            .unwrap()
            .id,
        certificate_id
    );
}

#[test]
fn activating_replacement_supersedes_old_certificate_and_emits_hot_reload() {
    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
    let control_plane = TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime);
    let certificate_id = CertificateId::new("cert-live").unwrap();
    let binding = HostnameBinding::new(
        Hostname::new("shop.example.com").unwrap(),
        CustomerAppId::new("storefront").unwrap(),
    );
    control_plane
        .import_certificate(
            CertificateRecord::new(
                certificate_id.clone(),
                CertificateProviderKind::Acme,
                CertificateStatus::Active,
                CertificateFingerprint::new("sha256:live").unwrap(),
                TlsInstant::from_unix_seconds(1_000),
                TlsInstant::from_unix_seconds(4_000_000),
                SecretMaterialRef::new("secrets/tls/cert-live").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(binding.clone()),
        )
        .unwrap();
    control_plane
        .queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();
    control_plane
        .begin_renewal(&certificate_id, CertificateId::new("cert-next").unwrap())
        .unwrap();

    let event = control_plane
        .activate_replacement(
            &certificate_id,
            CertificateRecord::new(
                CertificateId::new("cert-next").unwrap(),
                CertificateProviderKind::Acme,
                CertificateStatus::PendingIssuance,
                CertificateFingerprint::new("sha256:next").unwrap(),
                TlsInstant::from_unix_seconds(3_900_500),
                TlsInstant::from_unix_seconds(8_000_000),
                SecretMaterialRef::new("secrets/tls/cert-next").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(binding.clone()),
        )
        .unwrap();

    assert_eq!(event.certificate_id.as_str(), "cert-next");
    assert!(event.reloaded_without_restart);
    assert_eq!(
        control_plane
            .inventory()
            .active_for_hostname(&binding.hostname)
            .unwrap()
            .id
            .as_str(),
        "cert-next"
    );
    assert_eq!(
        control_plane
            .inventory()
            .record(&certificate_id)
            .unwrap()
            .status,
        CertificateStatus::Superseded
    );
    assert_eq!(control_plane.hot_reload_events().len(), 1);
}

#[test]
fn test_persistence_control_plane_persists_tls_state_between_instances() {
    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
    let path = temp_tls_state_path();
    let control_plane = TlsControlPlaneRuntime::with_test_persistence_control_plane_for_tests(
        runtime.clone(),
        path.to_string_lossy().to_string(),
    );
    let certificate_id = CertificateId::new("cert-persistent").unwrap();

    control_plane
        .import_certificate(
            CertificateRecord::new(
                certificate_id.clone(),
                CertificateProviderKind::Acme,
                CertificateStatus::Active,
                CertificateFingerprint::new("sha256:persistent").unwrap(),
                TlsInstant::from_unix_seconds(1_000),
                TlsInstant::from_unix_seconds(4_000_000),
                SecretMaterialRef::new("secrets/tls/cert-persistent").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(HostnameBinding::new(
                Hostname::new("persistent.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            )),
        )
        .unwrap();

    let second_control_plane =
        TlsControlPlaneRuntime::with_test_persistence_control_plane_for_tests(
            runtime,
            path.to_string_lossy().to_string(),
        );
    assert_eq!(
        second_control_plane
            .inventory()
            .record(&certificate_id)
            .unwrap()
            .status,
        CertificateStatus::Active
    );
}

#[test]
fn distributed_postgres_control_plane_persists_state_between_instances_when_database_url_is_available()
 {
    let Some(database_url) = std::env::var("DATABASE_URL").ok() else {
        eprintln!("skipping postgres TLS control-plane test: DATABASE_URL is not set");
        return;
    };

    let data_runtime = DataRuntime::from_config(&DatabaseConfig {
        driver: DatabaseDriver::Postgres,
        url: Some(SecretRef::Env {
            var: "DATABASE_URL".to_string(),
        }),
        schema: unique_tls_shared_state_namespace("tls_shared"),
        migrations_table: "_migrations".to_string(),
        ..DatabaseConfig::default()
    })
    .unwrap()
    .with_resolved_connection_url(database_url);

    let runtime = TlsRuntime::from_config(&acme_config(AcmeChallenge::Dns01, None));
    let namespace = unique_tls_shared_state_namespace("tls_shared_backend");
    let control_plane_one = TlsControlPlaneRuntime::with_distributed_postgres_control_plane(
        runtime.clone(),
        &data_runtime,
        namespace.clone(),
    )
    .unwrap();
    let control_plane_two = TlsControlPlaneRuntime::with_distributed_postgres_control_plane(
        runtime,
        &data_runtime,
        namespace,
    )
    .unwrap();
    let certificate_id = CertificateId::new("cert-shared").unwrap();

    control_plane_one
        .import_certificate(
            CertificateRecord::new(
                certificate_id.clone(),
                CertificateProviderKind::Acme,
                CertificateStatus::Active,
                CertificateFingerprint::new("sha256:shared").unwrap(),
                TlsInstant::from_unix_seconds(1_000),
                TlsInstant::from_unix_seconds(4_000_000),
                SecretMaterialRef::new("secrets/tls/cert-shared").unwrap(),
                CertificateStateStore::SharedSecrets,
            )
            .with_binding(HostnameBinding::new(
                Hostname::new("shared.example.com").unwrap(),
                CustomerAppId::new("storefront").unwrap(),
            )),
        )
        .unwrap();

    control_plane_one
        .queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();

    assert_eq!(
        control_plane_two
            .inventory()
            .record(&certificate_id)
            .unwrap()
            .status,
        CertificateStatus::RenewalDue
    );
    assert_eq!(control_plane_two.renewal_queue().len(), 1);
}

fn temp_tls_state_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "davenda-tls-{}-{}",
        std::process::id(),
        TlsInstant::from_unix_seconds(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
    ));
    path.push("state.json");
    path
}

fn unique_tls_shared_state_namespace(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        "{prefix}-{}-{timestamp}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
