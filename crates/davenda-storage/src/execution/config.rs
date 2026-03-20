use serde::Deserialize;
use thiserror::Error;
use url::Url;

const DEFAULT_S3_REGION: &str = "us-east-1";
const DEFAULT_SIGNED_URL_TTL_SECS: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreClientConfig {
    pub endpoint_url: Option<String>,
    pub bucket: String,
    pub region: String,
    pub credentials: ObjectStoreCredentials,
    pub signed_url_ttl_secs: u64,
    pub allow_http: bool,
    pub virtual_hosted_style_request: bool,
}

impl ObjectStoreClientConfig {
    pub fn new(
        bucket: impl Into<String>,
        region: impl Into<String>,
    ) -> Result<Self, ObjectStoreClientConfigError> {
        let bucket = bucket.into();
        if bucket.trim().is_empty() {
            return Err(ObjectStoreClientConfigError::MissingBucket);
        }

        let region = region.into();
        if region.trim().is_empty() {
            return Err(ObjectStoreClientConfigError::MissingRegion);
        }

        Ok(Self {
            endpoint_url: None,
            bucket,
            region,
            credentials: ObjectStoreCredentials::default(),
            signed_url_ttl_secs: DEFAULT_SIGNED_URL_TTL_SECS,
            allow_http: false,
            virtual_hosted_style_request: false,
        })
    }

    pub fn from_secret_value(secret_value: &str) -> Result<Self, ObjectStoreClientConfigError> {
        let secret_value = secret_value.trim();
        if secret_value.is_empty() {
            return Err(ObjectStoreClientConfigError::EmptySecret);
        }

        if let Ok(url) = Url::parse(secret_value) {
            return Self::from_legacy_url(url);
        }

        if let Ok(document) = serde_json::from_str::<ObjectStoreSecretDocument>(secret_value) {
            return Self::from_document(document);
        }

        if let Ok(document) = toml::from_str::<ObjectStoreSecretDocument>(secret_value) {
            return Self::from_document(document);
        }

        Err(ObjectStoreClientConfigError::UnsupportedSecretFormat)
    }

    pub fn from_structured_secret_value(
        secret_value: &str,
    ) -> Result<Self, ObjectStoreClientConfigError> {
        let secret_value = secret_value.trim();
        if secret_value.is_empty() {
            return Err(ObjectStoreClientConfigError::EmptySecret);
        }

        if let Ok(document) = serde_json::from_str::<ObjectStoreSecretDocument>(secret_value) {
            return Self::from_document(document);
        }

        if let Ok(document) = toml::from_str::<ObjectStoreSecretDocument>(secret_value) {
            return Self::from_document(document);
        }

        Err(ObjectStoreClientConfigError::UnsupportedStructuredSecretFormat)
    }

    pub fn with_endpoint_url(
        mut self,
        endpoint_url: impl Into<String>,
    ) -> Result<Self, ObjectStoreClientConfigError> {
        let endpoint_url = endpoint_url.into();
        let parsed = Url::parse(endpoint_url.trim()).map_err(|source| {
            ObjectStoreClientConfigError::InvalidEndpointUrl {
                value: endpoint_url.clone(),
                source,
            }
        })?;
        self.allow_http = parsed.scheme() == "http";
        self.endpoint_url = Some(normalize_endpoint_url(&parsed));
        Ok(self)
    }

    pub fn with_static_credentials(
        mut self,
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
    ) -> Result<Self, ObjectStoreClientConfigError> {
        let access_key_id = access_key_id.into();
        let secret_access_key = secret_access_key.into();
        if access_key_id.trim().is_empty() {
            return Err(ObjectStoreClientConfigError::MissingAccessKeyId);
        }
        if secret_access_key.trim().is_empty() {
            return Err(ObjectStoreClientConfigError::MissingSecretAccessKey);
        }
        self.credentials = ObjectStoreCredentials::Static {
            access_key_id,
            secret_access_key,
            session_token: None,
        };
        Ok(self)
    }

    pub fn with_session_token(mut self, token: impl Into<String>) -> Self {
        let token = token.into();
        self.credentials = match self.credentials {
            ObjectStoreCredentials::Static {
                access_key_id,
                secret_access_key,
                ..
            } => ObjectStoreCredentials::Static {
                access_key_id,
                secret_access_key,
                session_token: Some(token),
            },
            ObjectStoreCredentials::Environment => ObjectStoreCredentials::Environment,
        };
        self
    }

    pub fn with_signed_url_ttl_secs(mut self, signed_url_ttl_secs: u64) -> Self {
        self.signed_url_ttl_secs = signed_url_ttl_secs.max(1);
        self
    }

    pub fn with_virtual_hosted_style_request(mut self, virtual_hosted_style_request: bool) -> Self {
        self.virtual_hosted_style_request = virtual_hosted_style_request;
        self
    }

    fn from_document(
        document: ObjectStoreSecretDocument,
    ) -> Result<Self, ObjectStoreClientConfigError> {
        let bucket = document
            .bucket
            .or(document.bucket_name)
            .filter(|value| !value.trim().is_empty())
            .ok_or(ObjectStoreClientConfigError::MissingBucket)?;
        let region = document
            .region
            .or(document.default_region)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_S3_REGION.to_string());

        let mut config = Self::new(bucket, region)?;
        if let Some(endpoint_url) = document.endpoint_url.or(document.endpoint) {
            config = config.with_endpoint_url(endpoint_url)?;
        }
        config = config.with_signed_url_ttl_secs(
            document
                .signed_url_ttl_secs
                .unwrap_or(DEFAULT_SIGNED_URL_TTL_SECS),
        );
        config = config.with_virtual_hosted_style_request(
            document.virtual_hosted_style_request.unwrap_or(false),
        );
        if let Some(allow_http) = document.allow_http {
            config.allow_http = allow_http;
        }

        config.credentials = match (
            document.access_key_id,
            document.secret_access_key,
            document.session_token.or(document.token),
        ) {
            (Some(access_key_id), Some(secret_access_key), session_token) => {
                ObjectStoreCredentials::Static {
                    access_key_id,
                    secret_access_key,
                    session_token,
                }
            }
            (None, None, _) => ObjectStoreCredentials::Environment,
            (Some(_), None, _) => return Err(ObjectStoreClientConfigError::MissingSecretAccessKey),
            (None, Some(_), _) => return Err(ObjectStoreClientConfigError::MissingAccessKeyId),
        };

        Ok(config)
    }

    fn from_legacy_url(url: Url) -> Result<Self, ObjectStoreClientConfigError> {
        match url.scheme() {
            "s3" => {
                let bucket = url
                    .host_str()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or(ObjectStoreClientConfigError::MissingBucket)?;
                let region = url
                    .query_pairs()
                    .find_map(|(name, value)| {
                        (name == "region" || name == "aws_region").then_some(value.into_owned())
                    })
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| DEFAULT_S3_REGION.to_string());
                Self::new(bucket, region)
            }
            "http" | "https" => {
                let mut segments = url
                    .path_segments()
                    .ok_or(ObjectStoreClientConfigError::MissingBucket)?
                    .filter(|segment| !segment.is_empty());
                let bucket = segments
                    .next()
                    .ok_or(ObjectStoreClientConfigError::MissingBucket)?;
                let mut config = Self::new(bucket.to_string(), DEFAULT_S3_REGION.to_string())?;
                config.endpoint_url = Some(normalize_endpoint_url(&url));
                config.allow_http = url.scheme() == "http";
                Ok(config)
            }
            scheme => Err(ObjectStoreClientConfigError::UnsupportedUrlScheme {
                scheme: scheme.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ObjectStoreCredentials {
    #[default]
    Environment,
    Static {
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ObjectStoreClientConfigError {
    #[error("object-store secret is empty")]
    EmptySecret,
    #[error("object-store secret must be a supported URL, TOML, or JSON document")]
    UnsupportedSecretFormat,
    #[error("object-store secret must be a supported TOML or JSON document")]
    UnsupportedStructuredSecretFormat,
    #[error("object-store config requires a bucket name")]
    MissingBucket,
    #[error("object-store config requires a region")]
    MissingRegion,
    #[error("object-store config requires an access key id when a secret access key is set")]
    MissingAccessKeyId,
    #[error("object-store config requires a secret access key when an access key id is set")]
    MissingSecretAccessKey,
    #[error("object-store endpoint `{value}` is not a valid URL: {source}")]
    InvalidEndpointUrl {
        value: String,
        source: url::ParseError,
    },
    #[error("object-store URL scheme `{scheme}` is not supported")]
    UnsupportedUrlScheme { scheme: String },
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ObjectStoreSecretDocument {
    #[serde(default)]
    bucket: Option<String>,
    #[serde(default)]
    bucket_name: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    default_region: Option<String>,
    #[serde(default)]
    endpoint_url: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    access_key_id: Option<String>,
    #[serde(default)]
    secret_access_key: Option<String>,
    #[serde(default)]
    session_token: Option<String>,
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    signed_url_ttl_secs: Option<u64>,
    #[serde(default)]
    allow_http: Option<bool>,
    #[serde(default)]
    virtual_hosted_style_request: Option<bool>,
}

fn normalize_endpoint_url(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default();
    let mut normalized = format!("{}://{host}", url.scheme());
    if let Some(port) = url.port() {
        normalized.push(':');
        normalized.push_str(&port.to_string());
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_toml_secret() {
        let config = ObjectStoreClientConfig::from_secret_value(
            r#"
bucket = "runtime"
region = "eu-west-2"
endpoint_url = "https://storage.internal"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
session_token = "runtime-session"
signed_url_ttl_secs = 900
virtual_hosted_style_request = true
"#,
        )
        .unwrap();

        assert_eq!(
            config.endpoint_url.as_deref(),
            Some("https://storage.internal")
        );
        assert_eq!(config.bucket, "runtime");
        assert_eq!(config.region, "eu-west-2");
        assert_eq!(config.signed_url_ttl_secs, 900);
        assert!(config.virtual_hosted_style_request);
        assert_eq!(
            config.credentials,
            ObjectStoreCredentials::Static {
                access_key_id: "runtime-access".to_string(),
                secret_access_key: "runtime-secret".to_string(),
                session_token: Some("runtime-session".to_string()),
            }
        );
    }

    #[test]
    fn parses_legacy_http_url_secret() {
        let config =
            ObjectStoreClientConfig::from_secret_value("https://s3.internal/runtime").unwrap();

        assert_eq!(config.endpoint_url.as_deref(), Some("https://s3.internal"));
        assert_eq!(config.bucket, "runtime");
        assert_eq!(config.region, DEFAULT_S3_REGION);
        assert_eq!(config.credentials, ObjectStoreCredentials::Environment);
    }

    #[test]
    fn rejects_legacy_url_secret_for_structured_only_parser() {
        let error =
            ObjectStoreClientConfig::from_structured_secret_value("https://s3.internal/runtime")
                .unwrap_err();

        assert_eq!(
            error,
            ObjectStoreClientConfigError::UnsupportedStructuredSecretFormat
        );
    }

    #[test]
    fn rejects_partial_static_credentials() {
        let error = ObjectStoreClientConfig::from_secret_value(
            r#"
bucket = "runtime"
access_key_id = "runtime-access"
"#,
        )
        .unwrap_err();

        assert_eq!(error, ObjectStoreClientConfigError::MissingSecretAccessKey);
    }
}
