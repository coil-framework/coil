use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::{
    ActiveAssetManifest, AssetModelError, ContentFingerprint, DeploymentArtifact,
    DeploymentRelease, FingerprintAlgorithm, ReleaseId,
};
use davenda_storage::{StorageExecutor, StoragePlanner, StorageWriteReceipt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeAssetSource {
    source_path: PathBuf,
    bytes: Vec<u8>,
    artifact: DeploymentArtifact,
}

impl ThemeAssetSource {
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn artifact(&self) -> &DeploymentArtifact {
        &self.artifact
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeAssetPublicationPlan {
    release: DeploymentRelease,
    sources: BTreeMap<String, ThemeAssetSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeAssetPublicationReceipt {
    manifest: ActiveAssetManifest,
    writes: Vec<StorageWriteReceipt>,
}

impl ThemeAssetPublicationReceipt {
    pub fn manifest(&self) -> &ActiveAssetManifest {
        &self.manifest
    }

    pub fn writes(&self) -> &[StorageWriteReceipt] {
        &self.writes
    }
}

impl ThemeAssetPublicationPlan {
    pub fn from_roots<I, S, P>(
        release_id: ReleaseId,
        app_root: P,
        roots: I,
    ) -> Result<Self, AssetModelError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        P: AsRef<Path>,
    {
        let app_root = app_root.as_ref();
        let mut sources = Vec::new();

        for root in roots {
            collect_root_assets(app_root, root.as_ref(), &mut sources)?;
        }

        sources.sort_by(|left, right| {
            left.artifact
                .logical_path()
                .cmp(right.artifact.logical_path())
        });

        let release = DeploymentRelease::new(
            release_id,
            sources
                .iter()
                .map(|source| source.artifact().clone())
                .collect::<Vec<_>>(),
        )?;
        let sources = sources
            .into_iter()
            .map(|source| (source.artifact.logical_path().to_string(), source))
            .collect::<BTreeMap<_, _>>();

        Ok(Self { release, sources })
    }

    pub fn release(&self) -> &DeploymentRelease {
        &self.release
    }

    pub fn publish(
        &self,
        planner: &StoragePlanner,
        cdn_base_url: &str,
    ) -> Result<ActiveAssetManifest, AssetModelError> {
        self.release.publish(planner, cdn_base_url)
    }

    pub fn sync(
        &self,
        manifest: &ActiveAssetManifest,
        executor: &StorageExecutor,
    ) -> Result<Vec<StorageWriteReceipt>, AssetModelError> {
        let mut writes = Vec::new();

        for (logical_path, published) in manifest.entries() {
            let source = self.sources.get(logical_path).ok_or_else(|| {
                AssetModelError::MissingThemeAssetSource {
                    logical_path: logical_path.to_string(),
                }
            })?;
            let write =
                executor.execute_write(published.delivery().storage_plan(), source.bytes())?;
            writes.push(write);
        }

        Ok(writes)
    }

    pub fn publish_and_sync(
        &self,
        planner: &StoragePlanner,
        cdn_base_url: &str,
        executor: &StorageExecutor,
    ) -> Result<ThemeAssetPublicationReceipt, AssetModelError> {
        let manifest = self.publish(planner, cdn_base_url)?;
        let writes = self.sync(&manifest, executor)?;
        Ok(ThemeAssetPublicationReceipt { manifest, writes })
    }
}

fn collect_root_assets(
    app_root: &Path,
    source_root: &str,
    sources: &mut Vec<ThemeAssetSource>,
) -> Result<(), AssetModelError> {
    let source_root_path = app_root.join(source_root);
    if !source_root_path.exists() {
        return Err(AssetModelError::MissingThemeAssetRoot {
            root: source_root.to_string(),
        });
    }

    if !source_root_path.is_dir() {
        return Err(AssetModelError::MissingThemeAssetRoot {
            root: source_root.to_string(),
        });
    }

    collect_directory_assets(app_root, source_root, &source_root_path, sources)
}

fn collect_directory_assets(
    app_root: &Path,
    source_root: &str,
    current_dir: &Path,
    sources: &mut Vec<ThemeAssetSource>,
) -> Result<(), AssetModelError> {
    let mut entries = fs::read_dir(current_dir)
        .map_err(|error| AssetModelError::ThemeAssetReadFailed {
            path: current_dir.display().to_string(),
            message: error.to_string(),
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AssetModelError::ThemeAssetReadFailed {
            path: current_dir.display().to_string(),
            message: error.to_string(),
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type =
            entry
                .file_type()
                .map_err(|error| AssetModelError::ThemeAssetReadFailed {
                    path: path.display().to_string(),
                    message: error.to_string(),
                })?;

        if file_type.is_dir() {
            collect_directory_assets(app_root, source_root, &path, sources)?;
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let bytes = fs::read(&path).map_err(|error| AssetModelError::ThemeAssetReadFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let relative_path = path
            .strip_prefix(app_root.join(source_root))
            .expect("scanned asset path should always share the same source root");
        let relative_path = relative_manifest_path(relative_path);
        let logical_path = crate::normalize_manifest_path(
            "logical_path",
            format!("{source_root}/{relative_path}"),
        )?;
        let fingerprint = ContentFingerprint::new(
            FingerprintAlgorithm::Sha256,
            format!("{:x}", Sha256::digest(&bytes)),
        )?;
        let hashed_path = crate::normalize_manifest_path(
            "hashed_path",
            hashed_deployment_path(&logical_path, fingerprint.digest()),
        )?;
        let content_type = content_type_for_path(&path);
        let artifact = DeploymentArtifact::new(
            logical_path,
            hashed_path,
            fingerprint,
            content_type,
            bytes.len() as u64,
        )?;
        sources.push(ThemeAssetSource {
            source_path: path,
            bytes,
            artifact,
        });
    }

    Ok(())
}

fn relative_manifest_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn hashed_deployment_path(logical_path: &str, digest: &str) -> String {
    let path = Path::new(logical_path);
    let parent = path
        .parent()
        .map(|parent| parent.to_string_lossy().into_owned())
        .filter(|parent| !parent.is_empty());
    let file_name = path.file_name().unwrap().to_string_lossy();
    let hashed_file_name = match (path.file_stem(), path.extension()) {
        (Some(stem), Some(extension)) => format!(
            "{}.{}.{}",
            stem.to_string_lossy(),
            digest,
            extension.to_string_lossy()
        ),
        _ => format!("{file_name}.{digest}"),
    };

    match parent {
        Some(parent) => format!("deploy/{parent}/{hashed_file_name}"),
        None => format!("deploy/{hashed_file_name}"),
    }
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("css") => "text/css",
        Some("js") | Some("mjs") => "application/javascript",
        Some("json") | Some("map") => "application/json",
        Some("html") | Some("htm") => "text/html",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}
