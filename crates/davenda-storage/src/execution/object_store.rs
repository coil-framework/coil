use bytes::Bytes;
use object_store::ObjectStoreExt;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::path::Path as ObjectPath;
use object_store::signer::Signer;
use reqwest::Method;
use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{ObjectStoreClientConfig, ObjectStoreCredentials, StorageExecutionError};

#[derive(Debug, Clone)]
pub struct S3CompatibleObjectStoreClient {
    state: ObjectStoreClientState,
    signed_url_ttl: Duration,
}

impl S3CompatibleObjectStoreClient {
    pub fn new(config: ObjectStoreClientConfig) -> Self {
        let signed_url_ttl = Duration::from_secs(config.signed_url_ttl_secs.max(1));
        let state = match build_store(&config) {
            Ok(store) => ObjectStoreClientState::Ready { store },
            Err(message) => ObjectStoreClientState::Invalid { message },
        };

        Self {
            state,
            signed_url_ttl,
        }
    }

    pub fn put(&self, object_key: &str, bytes: &[u8]) -> Result<PathBuf, StorageExecutionError> {
        let path = object_path(object_key)?;
        let store = self.ready_store()?.clone();
        let payload = Bytes::copy_from_slice(bytes);
        run_object_store_future(async move { store.put(&path, payload.into()).await }).map_err(
            |message| StorageExecutionError::WriteFailed {
                path: object_key.to_string(),
                message,
            },
        )?;
        Ok(PathBuf::from(normalize_object_key(object_key)?))
    }

    pub fn get(&self, object_key: &str) -> Result<(PathBuf, Vec<u8>), StorageExecutionError> {
        let path = object_path(object_key)?;
        let store = self.ready_store()?.clone();
        let bytes = run_object_store_future(async move {
            let result = store.get(&path).await?;
            result.bytes().await
        })
        .map_err(|message| StorageExecutionError::ReadFailed {
            path: object_key.to_string(),
            message,
        })?;
        Ok((
            PathBuf::from(normalize_object_key(object_key)?),
            bytes.to_vec(),
        ))
    }

    pub fn signed_get_url(
        &self,
        object_key: &str,
    ) -> Result<SignedObjectUrl, StorageExecutionError> {
        let path = object_path(object_key)?;
        let store = self.ready_store()?.clone();
        let signed_url_ttl = self.signed_url_ttl;
        let signed_url = run_object_store_future(async move {
            store.signed_url(Method::GET, &path, signed_url_ttl).await
        })
        .map_err(|message| StorageExecutionError::SignedUrlGenerationFailed {
            object_key: object_key.to_string(),
            message,
        })?;
        let expires_at_unix_seconds = SystemTime::now()
            .checked_add(signed_url_ttl)
            .and_then(|instant| instant.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        Ok(SignedObjectUrl {
            object_key: normalize_object_key(object_key)?,
            signed_url: signed_url.to_string(),
            expires_at_unix_seconds,
        })
    }

    fn ready_store(&self) -> Result<&AmazonS3, StorageExecutionError> {
        match &self.state {
            ObjectStoreClientState::Ready { store } => Ok(store),
            ObjectStoreClientState::Invalid { message } => {
                Err(StorageExecutionError::InvalidObjectStoreConfiguration {
                    detail: message.clone(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedObjectUrl {
    pub object_key: String,
    pub signed_url: String,
    pub expires_at_unix_seconds: u64,
}

#[derive(Debug, Clone)]
enum ObjectStoreClientState {
    Ready { store: AmazonS3 },
    Invalid { message: String },
}

fn build_store(config: &ObjectStoreClientConfig) -> Result<AmazonS3, String> {
    validate_runtime_config(config)?;

    let mut builder = AmazonS3Builder::new()
        .with_bucket_name(config.bucket.clone())
        .with_region(config.region.clone())
        .with_virtual_hosted_style_request(config.virtual_hosted_style_request);
    if let Some(endpoint_url) = &config.endpoint_url {
        builder = builder
            .with_endpoint(endpoint_url.clone())
            .with_allow_http(config.allow_http);
    }
    match &config.credentials {
        ObjectStoreCredentials::Environment => {}
        ObjectStoreCredentials::Static {
            access_key_id,
            secret_access_key,
            session_token,
        } => {
            builder = builder
                .with_access_key_id(access_key_id.clone())
                .with_secret_access_key(secret_access_key.clone());
            if let Some(session_token) = session_token {
                builder = builder.with_token(session_token.clone());
            }
        }
    }
    builder.build().map_err(|error| error.to_string())
}

fn validate_runtime_config(config: &ObjectStoreClientConfig) -> Result<(), String> {
    if matches!(&config.credentials, ObjectStoreCredentials::Environment) {
        return Err(
            "runtime-backed object-store clients require explicit access_key_id and secret_access_key in the structured object-store secret"
                .to_string(),
        );
    }

    match config.endpoint_url.as_deref() {
        Some(endpoint_url) if endpoint_url.starts_with("http://") && !config.allow_http => {
            Err("object-store endpoint uses http but allow_http is not enabled".to_string())
        }
        None if config.allow_http => Err(
            "allow_http requires an explicit endpoint_url in the object-store config".to_string(),
        ),
        _ => Ok(()),
    }
}

fn run_object_store_future<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: Future<Output = Result<T, object_store::Error>> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => tokio::task::block_in_place(|| {
                handle.block_on(future).map_err(|error| error.to_string())
            }),
            tokio::runtime::RuntimeFlavor::CurrentThread => run_future_on_dedicated_runtime(future),
            _ => run_future_on_dedicated_runtime(future),
        },
        Err(_) => run_future_on_ephemeral_runtime(future),
    }
}

fn run_future_on_dedicated_runtime<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: Future<Output = Result<T, object_store::Error>> + Send + 'static,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(future).map_err(|error| error.to_string())
    })
    .join()
    .map_err(|_| "object-store worker thread panicked".to_string())?
}

fn run_future_on_ephemeral_runtime<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: Future<Output = Result<T, object_store::Error>> + Send + 'static,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(future).map_err(|error| error.to_string())
}

fn object_path(object_key: &str) -> Result<ObjectPath, StorageExecutionError> {
    ObjectPath::parse(normalize_object_key(object_key)?).map_err(|error| {
        StorageExecutionError::InvalidTargetPath {
            path: error.to_string(),
        }
    })
}

fn normalize_object_key(object_key: &str) -> Result<String, StorageExecutionError> {
    let mut parts = Vec::new();
    for component in std::path::Path::new(object_key).components() {
        match component {
            std::path::Component::Normal(part) => {
                parts.push(part.to_string_lossy().into_owned());
            }
            _ => {
                return Err(StorageExecutionError::InvalidTargetPath {
                    path: object_key.to_string(),
                });
            }
        }
    }

    if parts.is_empty() {
        return Err(StorageExecutionError::InvalidTargetPath {
            path: object_key.to_string(),
        });
    }

    Ok(parts.join("/"))
}
