use std::{env, future::Future, net::SocketAddr, path::PathBuf};

use openssl::{
    asn1::Asn1Time,
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    stack::Stack,
    x509::extension::SubjectAlternativeName,
    x509::{X509, X509Extension, X509NameBuilder, X509Req, X509ReqBuilder},
};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use sha2::Digest;

use crate::material::{CertificateMaterial, TlsMaterialProtector};
use crate::{
    CertificateFingerprint, CertificateId, CertificateProviderKind, CertificateRecord,
    CertificateStateStore, CertificateStatus, CloudflareEncryptionMode, HostnameBinding,
    SecretMaterialRef, TlsInstant, TlsModelError,
};

pub(crate) const LETS_ENCRYPT_DIRECTORY_URL: &str = lers::LETS_ENCRYPT_PRODUCTION_URL;
pub(crate) const CLOUDFLARE_API_BASE_URL: &str = "https://api.cloudflare.com/client/v4";
pub(crate) const CLOUDFLARE_ORIGIN_VALIDITY_DAYS: u64 = 5_475;

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ProviderSecretPayload {
    pub account_key_pem: Option<String>,
    pub cloudflare_api_token: Option<String>,
    pub cloudflare_service_key: Option<String>,
    pub cloudflare_email: Option<String>,
    pub cloudflare_api_key: Option<String>,
    pub http_challenge_directory: Option<String>,
    pub tls_alpn_bind_address: Option<String>,
    pub acme_directory_url: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProviderSecret {
    provider: CertificateProviderKind,
    raw: String,
    payload: ProviderSecretPayload,
}

impl ProviderSecret {
    pub(crate) fn resolve(
        provider: CertificateProviderKind,
        reference: Option<&str>,
    ) -> Result<Self, TlsModelError> {
        let reference = reference.ok_or_else(|| TlsModelError::MissingProviderCredential {
            provider: provider.to_string(),
        })?;
        let raw = resolve_secret(reference)?;
        let payload = serde_json::from_str::<ProviderSecretPayload>(&raw).unwrap_or_default();
        Ok(Self {
            provider,
            raw,
            payload,
        })
    }

    pub(crate) fn account_key_pem(&self) -> Result<PKey<Private>, TlsModelError> {
        let pem = self
            .payload
            .account_key_pem
            .as_deref()
            .or_else(|| self.raw.contains("BEGIN").then_some(self.raw.as_str()))
            .ok_or_else(|| TlsModelError::MissingProviderCredential {
                provider: self.provider.to_string(),
            })?;
        PKey::private_key_from_pem(pem.as_bytes())
            .map_err(|error| provider_error(self.provider, "parse_account_key", error))
    }

    pub(crate) fn acme_directory_url(&self) -> &str {
        self.payload
            .acme_directory_url
            .as_deref()
            .unwrap_or(LETS_ENCRYPT_DIRECTORY_URL)
    }

    pub(crate) fn http_challenge_directory(&self) -> Option<PathBuf> {
        self.payload
            .http_challenge_directory
            .as_deref()
            .map(PathBuf::from)
    }

    pub(crate) fn has_cloudflare_dns_credentials(&self) -> bool {
        self.cloudflare_api_token().is_some()
            || (self.payload.cloudflare_email.is_some()
                && self.payload.cloudflare_api_key.is_some())
    }

    pub(crate) fn has_http_challenge_directory(&self) -> bool {
        self.payload.http_challenge_directory.is_some()
    }

    pub(crate) fn has_tls_alpn_bind_address(&self) -> bool {
        self.payload.tls_alpn_bind_address.is_some()
    }

    pub(crate) fn tls_alpn_bind_address(&self) -> Result<Option<SocketAddr>, TlsModelError> {
        match self.payload.tls_alpn_bind_address.as_deref() {
            Some(address) => address.parse::<SocketAddr>().map(Some).map_err(|error| {
                provider_error(self.provider, "parse_tls_alpn_bind_address", error)
            }),
            None => Ok(None),
        }
    }

    pub(crate) fn cloudflare_dns_solver(
        &self,
    ) -> Result<lers::solver::dns::CloudflareDns01Solver, TlsModelError> {
        if let Some(token) = self.cloudflare_api_token() {
            return lers::solver::dns::CloudflareDns01Solver::new_with_token(token)
                .build()
                .map_err(|error| {
                    provider_error(self.provider, "build_cloudflare_dns_solver", error)
                });
        }

        match (
            self.payload.cloudflare_email.as_deref(),
            self.payload.cloudflare_api_key.as_deref(),
        ) {
            (Some(email), Some(api_key)) => {
                lers::solver::dns::CloudflareDns01Solver::new_with_auth_key(email, api_key)
                    .build()
                    .map_err(|error| {
                        provider_error(self.provider, "build_cloudflare_dns_solver", error)
                    })
            }
            _ => Err(TlsModelError::MissingProviderCredential {
                provider: self.provider.to_string(),
            }),
        }
    }

    pub(crate) fn cloudflare_headers(&self) -> Result<HeaderMap, TlsModelError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(token) = self.cloudflare_api_token() {
            let auth = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth).map_err(|error| {
                    provider_error(self.provider, "build_cloudflare_auth_header", error)
                })?,
            );
            return Ok(headers);
        }

        if let Some(service_key) = self.payload.cloudflare_service_key.as_deref() {
            headers.insert(
                HeaderName::from_static("x-auth-user-service-key"),
                HeaderValue::from_str(service_key).map_err(|error| {
                    provider_error(self.provider, "build_cloudflare_service_key_header", error)
                })?,
            );
            return Ok(headers);
        }

        if let (Some(email), Some(api_key)) = (
            self.payload.cloudflare_email.as_deref(),
            self.payload.cloudflare_api_key.as_deref(),
        ) {
            headers.insert(
                HeaderName::from_static("x-auth-email"),
                HeaderValue::from_str(email).map_err(|error| {
                    provider_error(self.provider, "build_cloudflare_email_header", error)
                })?,
            );
            headers.insert(
                HeaderName::from_static("x-auth-key"),
                HeaderValue::from_str(api_key).map_err(|error| {
                    provider_error(self.provider, "build_cloudflare_api_key_header", error)
                })?,
            );
            return Ok(headers);
        }

        Err(TlsModelError::MissingProviderCredential {
            provider: self.provider.to_string(),
        })
    }

    pub(crate) fn cloudflare_api_token(&self) -> Option<&str> {
        self.payload.cloudflare_api_token.as_deref().or_else(|| {
            (!self.raw.trim().is_empty()
                && !self.raw.contains("BEGIN")
                && !self.raw.trim_start().starts_with('{'))
            .then_some(self.raw.as_str())
        })
    }
}

pub(crate) fn run_blocking<T>(
    provider: CertificateProviderKind,
    operation: &'static str,
    future: impl Future<Output = Result<T, TlsModelError>> + 'static,
) -> Result<T, TlsModelError>
where
    T: Send + 'static,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| provider_error(provider, operation, error))?;
    runtime.block_on(future)
}

pub(crate) fn provider_error(
    provider: CertificateProviderKind,
    operation: &'static str,
    error: impl std::fmt::Display,
) -> TlsModelError {
    TlsModelError::ProviderRequestFailed {
        provider: provider.to_string(),
        operation,
        reason: error.to_string(),
    }
}

pub(crate) fn build_record(
    provider: CertificateProviderKind,
    certificate_id: CertificateId,
    bindings: &[HostnameBinding],
    state_store: CertificateStateStore,
    cloudflare_mode: Option<CloudflareEncryptionMode>,
    issued_at: TlsInstant,
    certificate_chain: String,
    private_key: String,
    protector: &TlsMaterialProtector,
) -> Result<CertificateRecord, TlsModelError> {
    let material = CertificateMaterial::new(certificate_chain, private_key)?;
    let not_after = certificate_not_after(provider, material.certificate_chain_pem().as_str())?;
    let encrypted = protector.encrypt(&material)?;
    let mut record = CertificateRecord::new(
        certificate_id.clone(),
        provider,
        CertificateStatus::Active,
        certificate_fingerprint(&material)?,
        issued_at,
        not_after,
        SecretMaterialRef::new(format!("secrets/tls/{certificate_id}"))?,
        state_store,
    )
    .with_material(encrypted);

    for binding in bindings.iter().cloned() {
        record = record.with_binding(binding);
    }

    if let Some(mode) = cloudflare_mode {
        record = record.with_cloudflare_mode(mode);
    }

    Ok(record)
}

fn certificate_not_after(
    provider: CertificateProviderKind,
    certificate_chain: &str,
) -> Result<TlsInstant, TlsModelError> {
    let certificates = X509::stack_from_pem(certificate_chain.as_bytes())
        .map_err(|error| provider_error(provider, "parse_certificate_chain", error))?;
    let leaf = certificates
        .first()
        .ok_or_else(|| TlsModelError::InvalidCertificateMaterial {
            field: "certificate_chain_pem",
            reason: "certificate chain did not contain a leaf certificate".to_string(),
        })?;
    let epoch = Asn1Time::from_unix(0)
        .map_err(|error| provider_error(provider, "parse_not_after", error))?;
    let diff = epoch
        .diff(leaf.not_after())
        .map_err(|error| provider_error(provider, "parse_not_after", error))?;
    let days = u64::try_from(diff.days).map_err(|_| TlsModelError::InvalidCertificateMaterial {
        field: "certificate_chain_pem",
        reason: "certificate `notAfter` is before the unix epoch".to_string(),
    })?;
    let secs = u64::try_from(diff.secs).map_err(|_| TlsModelError::InvalidCertificateMaterial {
        field: "certificate_chain_pem",
        reason: "certificate `notAfter` is before the unix epoch".to_string(),
    })?;
    let total_seconds = days.saturating_mul(24 * 60 * 60).saturating_add(secs);
    Ok(TlsInstant::from_unix_seconds(total_seconds))
}

pub(crate) fn decrypt_material(
    control_plane: &crate::runtime::TlsControlPlaneRuntime,
    protector: &TlsMaterialProtector,
    certificate_id: &CertificateId,
) -> Result<CertificateMaterial, TlsModelError> {
    let record = control_plane
        .inventory()
        .record(certificate_id)
        .cloned()
        .ok_or_else(|| TlsModelError::UnknownCertificate {
            certificate_id: certificate_id.to_string(),
        })?;
    let material = record
        .material
        .ok_or_else(|| TlsModelError::MissingCertificateMaterial {
            certificate_id: certificate_id.to_string(),
        })?;
    protector.decrypt(&material)
}

pub(crate) fn certificate_fingerprint(
    material: &CertificateMaterial,
) -> Result<CertificateFingerprint, TlsModelError> {
    let digest = sha2::Sha256::digest(material.certificate_chain_pem().as_str().as_bytes());
    CertificateFingerprint::new(format!("sha256:{:x}", digest))
}

pub(crate) fn generate_private_key(
    provider: CertificateProviderKind,
) -> Result<PKey<Private>, TlsModelError> {
    let rsa = Rsa::generate(2048)
        .map_err(|error| provider_error(provider, "generate_private_key", error))?;
    PKey::from_rsa(rsa).map_err(|error| provider_error(provider, "generate_private_key", error))
}

pub(crate) fn private_key_to_pem(
    provider: CertificateProviderKind,
    private_key: &PKey<Private>,
) -> Result<String, TlsModelError> {
    let pem = private_key
        .private_key_to_pem_pkcs8()
        .map_err(|error| provider_error(provider, "encode_private_key", error))?;
    String::from_utf8(pem).map_err(|error| provider_error(provider, "encode_private_key", error))
}

pub(crate) fn build_certificate_request(
    provider: CertificateProviderKind,
    private_key: &PKey<Private>,
    bindings: &[HostnameBinding],
) -> Result<String, TlsModelError> {
    let mut subject = X509NameBuilder::new()
        .map_err(|error| provider_error(provider, "build_subject_name", error))?;
    subject
        .append_entry_by_text(
            "CN",
            challenge_domain(
                bindings
                    .first()
                    .map(|binding| binding.hostname.as_str())
                    .unwrap_or("localhost"),
            )
            .as_str(),
        )
        .map_err(|error| provider_error(provider, "build_subject_name", error))?;

    let mut builder =
        X509ReqBuilder::new().map_err(|error| provider_error(provider, "build_csr", error))?;
    builder
        .set_pubkey(private_key)
        .map_err(|error| provider_error(provider, "build_csr", error))?;
    builder
        .set_subject_name(&subject.build())
        .map_err(|error| provider_error(provider, "build_csr", error))?;

    let mut san = SubjectAlternativeName::new();
    for binding in bindings {
        san.dns(binding.hostname.as_str());
    }
    let san_extension: X509Extension = san
        .build(&builder.x509v3_context(None))
        .map_err(|error| provider_error(provider, "build_csr", error))?;
    let mut extensions =
        Stack::new().map_err(|error| provider_error(provider, "build_csr", error))?;
    extensions
        .push(san_extension)
        .map_err(|error| provider_error(provider, "build_csr", error))?;
    builder
        .add_extensions(&extensions)
        .map_err(|error| provider_error(provider, "build_csr", error))?;
    builder
        .sign(private_key, MessageDigest::sha256())
        .map_err(|error| provider_error(provider, "build_csr", error))?;

    let request: X509Req = builder.build();
    let pem = request
        .to_pem()
        .map_err(|error| provider_error(provider, "encode_csr", error))?;
    String::from_utf8(pem).map_err(|error| provider_error(provider, "encode_csr", error))
}

pub(crate) fn challenge_domain(hostname: &str) -> String {
    hostname.trim_start_matches("*.").to_string()
}

pub(crate) fn resolve_secret(reference: &str) -> Result<String, TlsModelError> {
    if let Some(var) = reference.strip_prefix("env:") {
        return env::var(var).map_err(|_| TlsModelError::MissingProviderCredential {
            provider: format!("env:{var}"),
        });
    }

    if reference.starts_with("secret-manager:") {
        return Err(TlsModelError::MissingProviderCredential {
            provider: reference.to_string(),
        });
    }

    Ok(reference.to_string())
}
