use coil_config::{AcmeChallenge, TlsConfig, TlsMode, TlsProvider};
use coil_tls::{
    AcmeTlsCertificateExecutor, CertificateId, CertificateProviderKind, CertificateStateStore,
    ChallengeStrategy, CloudflareTlsCertificateExecutor, CustomerAppId, EdgeMode, Hostname,
    HostnameBinding, IssuancePlan, TlsCertificateExecutor, TlsControlPlaneRuntime, TlsInstant,
    TlsMaterialProtector, TlsModelError, TlsRuntime,
};

fn binding(hostname: &str) -> HostnameBinding {
    HostnameBinding::new(
        Hostname::new(hostname).unwrap(),
        CustomerAppId::new("showcase-events").unwrap(),
    )
}

#[test]
fn real_acme_executor_fails_closed_without_provider_credentials() {
    let runtime = TlsRuntime::from_config(&TlsConfig {
        mode: TlsMode::Acme,
        challenge: Some(AcmeChallenge::Dns01),
        provider: None,
        account_secret: None,
    });
    let control_plane = TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime.clone());
    let executor = AcmeTlsCertificateExecutor::new(
        control_plane,
        TlsMaterialProtector::from_seed("real-acme-executor-test").unwrap(),
        runtime.account_secret_ref.clone(),
    );
    let plan = runtime
        .planner()
        .issue_for_bindings(vec![binding("www.example.com")])
        .unwrap();

    let error = executor
        .issue_certificate(
            &plan,
            CertificateId::new("cert-real-acme").unwrap(),
            TlsInstant::from_unix_seconds(1_700_000_000),
        )
        .unwrap_err();

    assert_eq!(
        error,
        TlsModelError::MissingProviderCredential {
            provider: CertificateProviderKind::Acme.to_string(),
        }
    );
}

#[test]
fn real_cloudflare_dns_executor_rejects_non_dns_challenges() {
    let runtime = TlsRuntime::from_config(&TlsConfig {
        mode: TlsMode::Acme,
        challenge: Some(AcmeChallenge::Dns01),
        provider: Some(TlsProvider::CloudflareDns),
        account_secret: None,
    });
    let control_plane = TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime);
    let executor = CloudflareTlsCertificateExecutor::new(
        CertificateProviderKind::CloudflareDns,
        control_plane,
        TlsMaterialProtector::from_seed("real-cloudflare-dns-executor-test").unwrap(),
        Some("{}".to_string()),
    );
    let plan = IssuancePlan {
        edge_mode: EdgeMode::DirectTermination,
        provider: CertificateProviderKind::CloudflareDns,
        challenge: Some(ChallengeStrategy::Http01),
        state_store: CertificateStateStore::SharedSecrets,
        bindings: vec![binding("www.example.com")],
        shared_across_nodes: false,
        requires_hot_reload: true,
        account_secret: Some("{}".to_string()),
        cloudflare_mode: None,
    };

    let error = executor
        .issue_certificate(
            &plan,
            CertificateId::new("cert-real-cloudflare-dns").unwrap(),
            TlsInstant::from_unix_seconds(1_700_000_000),
        )
        .unwrap_err();

    assert_eq!(
        error,
        TlsModelError::UnsupportedProviderChallenge {
            provider: CertificateProviderKind::CloudflareDns.to_string(),
            challenge: ChallengeStrategy::Http01.to_string(),
        }
    );
}

#[test]
fn real_cloudflare_origin_executor_fails_closed_without_provider_credentials() {
    let runtime = TlsRuntime::from_config(&TlsConfig {
        mode: TlsMode::CloudflareOrigin,
        challenge: None,
        provider: Some(TlsProvider::CloudflareOriginCa),
        account_secret: None,
    });
    let control_plane = TlsControlPlaneRuntime::in_memory_control_plane_for_tests(runtime.clone());
    let executor = CloudflareTlsCertificateExecutor::new(
        CertificateProviderKind::CloudflareOriginCa,
        control_plane,
        TlsMaterialProtector::from_seed("real-cloudflare-origin-executor-test").unwrap(),
        runtime.account_secret_ref.clone(),
    );
    let plan = runtime
        .planner()
        .issue_for_bindings(vec![binding("origin.example.com")])
        .unwrap();

    let error = executor
        .issue_certificate(
            &plan,
            CertificateId::new("cert-real-cloudflare-origin").unwrap(),
            TlsInstant::from_unix_seconds(1_700_000_000),
        )
        .unwrap_err();

    assert_eq!(
        error,
        TlsModelError::MissingProviderCredential {
            provider: CertificateProviderKind::CloudflareOriginCa.to_string(),
        }
    );
}
