use anyhow::{Context, Result, bail};
use coil_app::CustomerExtension;
use coil_wasm::{
    ContractVersion, ExtensionArtifactSource, ExtensionConfigSchema, ExtensionInstallation,
    ExtensionManifest, ExtensionPackage, ExtensionPoint, ExtensionPointKind, HandlerId,
    HandlerInstallation, HandlerManifest, HostGrantSet, RenderHookExtensionPoint, ResourceLimits,
    TypedExecutionOutput, TypedMetadata,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn augment_manifest_with_extensions(
    manifest_path: &Path,
    mut manifest: coil_app::CustomerAppManifest,
) -> Result<coil_app::CustomerAppManifest> {
    for extension in load_declared_extensions(manifest_path)? {
        manifest = manifest.with_extension(extension);
    }
    manifest
        .validate()
        .context("Shoppr manifest extension configuration is invalid")?;
    Ok(manifest)
}

pub(super) fn load_extension_packages(
    app_root: &Path,
    extension_directory: &Path,
    manifest_path: &Path,
) -> Result<Vec<ExtensionPackage>> {
    let document = ShopprExtensionInstallDocument::from_file(manifest_path)?;
    document
        .extensions
        .into_iter()
        .map(|extension| load_extension_package(app_root, extension_directory, &extension.id))
        .collect()
}

#[derive(Debug, Default, Deserialize)]
struct ShopprExtensionInstallDocument {
    #[serde(default)]
    extensions: Vec<ShopprInstalledExtensionDocument>,
}

impl ShopprExtensionInstallDocument {
    fn from_file(path: &Path) -> Result<Self> {
        let input = fs::read_to_string(path).with_context(|| {
            format!(
                "failed to read Shoppr extension installs from `{}`",
                path.display()
            )
        })?;
        toml::from_str(&input).with_context(|| {
            format!(
                "failed to parse Shoppr extension installs from `{}`",
                path.display()
            )
        })
    }
}

#[derive(Debug, Deserialize)]
struct ShopprInstalledExtensionDocument {
    id: String,
    package_version: String,
    artifact_sha256: String,
    customer_app_id: String,
    #[serde(default)]
    handlers: Vec<ShopprInstalledHandlerDocument>,
}

#[derive(Debug, Deserialize)]
struct ShopprInstalledHandlerDocument {
    id: String,
    #[serde(default)]
    grants: Vec<String>,
}

fn load_declared_extensions(manifest_path: &Path) -> Result<Vec<CustomerExtension>> {
    let document = ShopprExtensionInstallDocument::from_file(manifest_path)?;
    document
        .extensions
        .into_iter()
        .map(ShopprInstalledExtensionDocument::into_model)
        .collect()
}

impl ShopprInstalledExtensionDocument {
    fn into_model(self) -> Result<CustomerExtension> {
        let handlers = self
            .handlers
            .into_iter()
            .map(ShopprInstalledHandlerDocument::into_model)
            .collect::<Result<Vec<_>>>()?;
        let installation = ExtensionInstallation::new(self.customer_app_id, handlers)
            .context("failed to build Shoppr extension installation")?;
        CustomerExtension::new(
            self.id,
            parse_contract_version(&self.package_version)?,
            self.artifact_sha256,
            installation,
        )
        .context("failed to build Shoppr customer extension")
    }
}

impl ShopprInstalledHandlerDocument {
    fn into_model(self) -> Result<HandlerInstallation> {
        Ok(HandlerInstallation::new(
            HandlerId::new(self.id).context("invalid Shoppr extension handler id")?,
            parse_grants(&self.grants)?,
        ))
    }
}

#[derive(Debug, Deserialize)]
struct ShopprExtensionPackageDocument {
    publisher: String,
    artifact: String,
    artifact_sha256: String,
    source_wat: String,
    manifest: ShopprExtensionManifestDocument,
    #[serde(default)]
    handlers: Vec<ShopprExtensionHandlerDocument>,
}

#[derive(Debug, Deserialize)]
struct ShopprExtensionManifestDocument {
    id: String,
    display_name: String,
    version: String,
    host_api_version: String,
}

#[derive(Debug, Deserialize)]
struct ShopprExtensionHandlerDocument {
    id: String,
    export: String,
    point: String,
    target: String,
    #[serde(default)]
    grants: Vec<String>,
}

fn load_extension_package(
    app_root: &Path,
    extension_directory: &Path,
    extension_id: &str,
) -> Result<ExtensionPackage> {
    let package_dir = app_root.join("extensions").join(extension_id);
    let package_path = package_dir.join("package.toml");
    let input = fs::read_to_string(&package_path).with_context(|| {
        format!(
            "failed to read Shoppr extension package `{}`",
            package_path.display()
        )
    })?;
    let document: ShopprExtensionPackageDocument = toml::from_str(&input).with_context(|| {
        format!(
            "failed to parse Shoppr extension package `{}`",
            package_path.display()
        )
    })?;

    if document.manifest.id != extension_id {
        bail!(
            "Shoppr extension package `{}` declares id `{}` instead of `{}`",
            package_path.display(),
            document.manifest.id,
            extension_id
        );
    }

    compile_demo_artifact(&package_dir, extension_directory, &document)
        .with_context(|| format!("failed to compile Shoppr extension artifact `{extension_id}`"))?;

    let handlers = document
        .handlers
        .into_iter()
        .map(ShopprExtensionHandlerDocument::into_model)
        .collect::<Result<Vec<_>>>()?;
    let manifest = ExtensionManifest::new(
        coil_wasm::ExtensionId::new(document.manifest.id)
            .context("invalid Shoppr extension package id")?,
        document.manifest.display_name,
        parse_contract_version(&document.manifest.version)?,
        parse_contract_version(&document.manifest.host_api_version)?,
        ResourceLimits::baseline_for(ExtensionPointKind::RenderHook),
        handlers,
    )
    .context("failed to build Shoppr extension manifest")?;

    ExtensionPackage::new(
        document.publisher,
        manifest,
        ExtensionArtifactSource::local_path(document.artifact)
            .context("invalid Shoppr extension artifact path")?,
        document.artifact_sha256,
        ExtensionConfigSchema::new(1, Vec::new())
            .expect("empty Shoppr extension config schema should be valid"),
    )
    .context("failed to build Shoppr extension package")
}

impl ShopprExtensionHandlerDocument {
    fn into_model(self) -> Result<HandlerManifest> {
        Ok(HandlerManifest::new(
            HandlerId::new(self.id).context("invalid Shoppr extension handler id")?,
            self.export,
            parse_extension_point(&self.point, &self.target)?,
            parse_grants(&self.grants)?,
        )?)
    }
}

fn compile_demo_artifact(
    package_dir: &Path,
    extension_directory: &Path,
    document: &ShopprExtensionPackageDocument,
) -> Result<()> {
    let Some(handler) = document.handlers.first() else {
        bail!("Shoppr extension package must declare at least one handler");
    };
    if document.handlers.len() != 1 {
        bail!("Shoppr demo extension currently supports exactly one installed handler");
    }
    if !handler.point.eq_ignore_ascii_case("render-hook") {
        bail!(
            "Shoppr demo extension compiler currently supports render-hook handlers only, got `{}`",
            handler.point
        );
    }

    let typed_output = TypedExecutionOutput::render_hook(
        200,
        format!(
            "<aside data-extension=\"{}\">Shoppr Waitlist Tools is active through the bounded runtime-installed WASM path.</aside>",
            document.manifest.id
        ),
        TypedMetadata::new()
            .with_description("Shoppr runtime-installed WASM render hook output")
            .expect("static Shoppr metadata should be valid"),
        None,
    )
    .expect("static Shoppr render hook output should be valid");
    let typed_output_bytes = typed_output
        .encode()
        .context("failed to encode Shoppr render hook typed output")?;
    let packed_len = (typed_output_bytes.len() as u64) << 32;

    let wat_path = package_dir.join(&document.source_wat);
    let wat_template = fs::read_to_string(&wat_path).with_context(|| {
        format!(
            "failed to read Shoppr extension source `{}`",
            wat_path.display()
        )
    })?;
    let wat_module = wat_template
        .replace(
            "__COIL_TYPED_OUTPUT__",
            &wat_string_literal(&typed_output_bytes),
        )
        .replace("__COIL_TYPED_OUTPUT_PACKED__", &packed_len.to_string())
        .replace("__COIL_HANDLER_EXPORT__", &handler.export);
    let artifact_bytes = wat::parse_str(&wat_module)
        .with_context(|| format!("failed to compile Shoppr WAT `{}`", wat_path.display()))?;

    let artifact_path = extension_directory.join(&document.artifact);
    if let Some(parent) = artifact_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create Shoppr extension artifact directory `{}`",
                parent.display()
            )
        })?;
    }
    fs::write(&artifact_path, artifact_bytes).with_context(|| {
        format!(
            "failed to write Shoppr extension artifact `{}`",
            artifact_path.display()
        )
    })?;
    Ok(())
}

fn parse_extension_point(point: &str, target: &str) -> Result<ExtensionPoint> {
    match point.to_ascii_lowercase().as_str() {
        "render-hook" | "render_hook" => Ok(ExtensionPoint::RenderHook(
            RenderHookExtensionPoint::new(target)
                .context("invalid Shoppr render-hook extension target")?,
        )),
        other => bail!("unsupported Shoppr extension point `{other}`"),
    }
}

fn parse_grants(values: &[String]) -> Result<HostGrantSet> {
    let grants = HostGrantSet::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() {
            continue;
        }
        bail!("unsupported Shoppr extension grant `{normalized}`");
    }
    Ok(grants)
}

fn parse_contract_version(input: &str) -> Result<ContractVersion> {
    let mut parts = input.split('.');
    let parse_part = |label: &'static str, value: Option<&str>| -> Result<u16> {
        let value = value.ok_or_else(|| anyhow::anyhow!("missing {label} version component"))?;
        value
            .parse::<u16>()
            .with_context(|| format!("invalid {label} version component `{value}`"))
    };
    let major = parse_part("major", parts.next())?;
    let minor = parse_part("minor", parts.next())?;
    let patch = parse_part("patch", parts.next())?;
    if parts.next().is_some() {
        bail!("invalid contract version `{input}`");
    }
    Ok(ContractVersion::new(major, minor, patch))
}

pub(crate) fn compiled_demo_artifact_sha256(app_root: &Path, extension_id: &str) -> Result<String> {
    let package_path = app_root
        .join("extensions")
        .join(extension_id)
        .join("package.toml");
    let input = fs::read_to_string(&package_path).with_context(|| {
        format!(
            "failed to read Shoppr extension package `{}`",
            package_path.display()
        )
    })?;
    let document: ShopprExtensionPackageDocument = toml::from_str(&input).with_context(|| {
        format!(
            "failed to parse Shoppr extension package `{}`",
            package_path.display()
        )
    })?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let extension_directory = std::env::temp_dir().join(format!("shoppr-extension-{unique}"));
    compile_demo_artifact(
        package_path.parent().expect("package.toml has a parent"),
        &extension_directory,
        &document,
    )?;
    let bytes = fs::read(extension_directory.join(&document.artifact)).with_context(|| {
        format!("failed to read compiled Shoppr extension artifact for `{extension_id}`")
    })?;
    let _ = fs::remove_dir_all(&extension_directory);
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn wat_string_literal(bytes: &[u8]) -> String {
    let mut literal = String::with_capacity(bytes.len() * 4);
    for byte in bytes {
        literal.push_str(&format!("\\{:02x}", byte));
    }
    literal
}
