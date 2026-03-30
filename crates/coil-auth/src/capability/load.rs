use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use zanzibar::{NamespaceConfig, Schema};

use crate::{
    Capability, Namespace, Relation, RelationRule, default_capability_bindings, default_manifest,
    default_schema,
};

use super::*;

#[derive(Debug, Clone)]
pub struct LoadedAuthModelPackage {
    manifest: AuthModelManifest,
    schema: Schema,
    capability_bindings: HashMap<Capability, CapabilityBinding>,
}

impl AuthModelPackage for LoadedAuthModelPackage {
    fn manifest(&self) -> &AuthModelManifest {
        &self.manifest
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn capability_bindings(&self) -> &HashMap<Capability, CapabilityBinding> {
        &self.capability_bindings
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthModelPackageLoadError {
    Io {
        path: PathBuf,
        message: String,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    ManifestNameMismatch {
        expected: String,
        actual: String,
    },
    MissingImportBase {
        package: String,
    },
    UnsupportedImportFanIn {
        package: String,
        imports: Vec<String>,
    },
    UnsupportedModelSyntax {
        path: PathBuf,
        line: usize,
        message: String,
    },
    UnknownCapability {
        package: String,
        capability: String,
    },
    UnknownNamespace {
        package: String,
        namespace: String,
    },
    UnknownRelation {
        package: String,
        relation: String,
    },
    UnknownPackage {
        package: String,
        expected_path: PathBuf,
    },
}

impl fmt::Display for AuthModelPackageLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    f,
                    "failed to read auth package file `{}`: {message}",
                    path.display()
                )
            }
            Self::Parse { path, message } => {
                write!(
                    f,
                    "failed to parse auth package file `{}`: {message}",
                    path.display()
                )
            }
            Self::ManifestNameMismatch { expected, actual } => write!(
                f,
                "auth package name mismatch: configured `{expected}` but package manifest declares `{actual}`"
            ),
            Self::MissingImportBase { package } => write!(
                f,
                "extend-mode auth package `{package}` must import a base package"
            ),
            Self::UnsupportedImportFanIn { package, imports } => write!(
                f,
                "auth package `{package}` imports multiple base packages ({}) which the current loader does not support yet",
                imports.join(", ")
            ),
            Self::UnsupportedModelSyntax {
                path,
                line,
                message,
            } => write!(
                f,
                "unsupported auth model syntax in `{}` at line {}: {message}",
                path.display(),
                line
            ),
            Self::UnknownCapability {
                package,
                capability,
            } => write!(
                f,
                "auth package `{package}` references unsupported capability `{capability}`"
            ),
            Self::UnknownNamespace { package, namespace } => write!(
                f,
                "auth package `{package}` references unsupported namespace `{namespace}`"
            ),
            Self::UnknownRelation { package, relation } => write!(
                f,
                "auth package `{package}` references unsupported relation `{relation}`"
            ),
            Self::UnknownPackage {
                package,
                expected_path,
            } => write!(
                f,
                "auth package `{package}` was not found under `{}`",
                expected_path.display()
            ),
        }
    }
}

impl Error for AuthModelPackageLoadError {}

#[derive(Debug, Deserialize)]
struct AuthPackageDocument {
    name: String,
    version: String,
    mode: String,
    storage_schema_version: u32,
    model_version: u32,
    capability_binding_version: u32,
    #[serde(default)]
    imports: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CapabilityBindingsDocument {
    #[serde(default)]
    bindings: HashMap<String, CapabilityBindingDocument>,
}

#[derive(Debug, Deserialize)]
struct CapabilityBindingDocument {
    resource_type: String,
    permission: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelSection {
    Relations,
    Permissions,
}

pub fn load_auth_model_package_at(
    name: impl AsRef<str>,
    app_root: impl AsRef<Path>,
) -> Result<LoadedAuthModelPackage, AuthModelPackageLoadError> {
    load_auth_model_package_inner(name.as_ref(), app_root.as_ref())
}

pub fn load_auth_model_package_selection_at(
    name: impl AsRef<str>,
    app_root: impl AsRef<Path>,
) -> Result<AuthModelPackageSelection, AuthModelPackageLoadError> {
    Ok(AuthModelPackageSelection::new(load_auth_model_package_at(
        name, app_root,
    )?))
}

fn load_auth_model_package_inner(
    name: &str,
    app_root: &Path,
) -> Result<LoadedAuthModelPackage, AuthModelPackageLoadError> {
    if name == default_manifest().name {
        return Ok(LoadedAuthModelPackage {
            manifest: default_manifest(),
            schema: default_schema(),
            capability_bindings: default_capability_bindings(),
        });
    }

    let package_root = app_root.join("auth").join(name);
    if !package_root.is_dir() {
        return Err(AuthModelPackageLoadError::UnknownPackage {
            package: name.to_string(),
            expected_path: package_root,
        });
    }

    let manifest_path = package_root.join("package.toml");
    let manifest_document = read_toml::<AuthPackageDocument>(&manifest_path)?;
    if manifest_document.name != name {
        return Err(AuthModelPackageLoadError::ManifestNameMismatch {
            expected: name.to_string(),
            actual: manifest_document.name,
        });
    }

    let manifest = AuthModelManifest {
        name: manifest_document.name,
        version: parse_package_version(&manifest_document.version, &manifest_path)?,
        mode: parse_package_mode(&manifest_document.mode, &manifest_path)?,
        storage_schema_version: manifest_document.storage_schema_version,
        model_version: manifest_document.model_version,
        capability_binding_version: manifest_document.capability_binding_version,
        imports: manifest_document.imports.clone(),
    };

    match manifest.mode {
        PackageMode::Replace => {
            let schema =
                load_model_schema(&package_root.join("model.auth"), &manifest.name, None, true)?;
            let capability_bindings =
                load_capability_bindings(&package_root.join("capabilities.toml"), &manifest.name)?;

            Ok(LoadedAuthModelPackage {
                manifest,
                schema,
                capability_bindings,
            })
        }
        PackageMode::Extend => {
            if manifest.imports.is_empty() {
                return Err(AuthModelPackageLoadError::MissingImportBase {
                    package: manifest.name,
                });
            }
            if manifest.imports.len() > 1 {
                return Err(AuthModelPackageLoadError::UnsupportedImportFanIn {
                    package: manifest.name,
                    imports: manifest.imports.clone(),
                });
            }

            let imported = load_auth_model_package_inner(&manifest.imports[0], app_root)?;
            let schema = load_model_schema(
                &package_root.join("model.auth"),
                &manifest.name,
                Some(imported.schema()),
                false,
            )?;
            let mut capability_bindings = imported.capability_bindings().clone();
            capability_bindings.extend(load_capability_bindings(
                &package_root.join("capabilities.toml"),
                &manifest.name,
            )?);

            Ok(LoadedAuthModelPackage {
                manifest,
                schema,
                capability_bindings,
            })
        }
    }
}

fn read_toml<T>(path: &Path) -> Result<T, AuthModelPackageLoadError>
where
    T: for<'de> Deserialize<'de>,
{
    let input = fs::read_to_string(path).map_err(|error| AuthModelPackageLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    toml::from_str(&input).map_err(|error| AuthModelPackageLoadError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn parse_package_version(
    value: &str,
    path: &Path,
) -> Result<PackageVersion, AuthModelPackageLoadError> {
    let mut components = value.split('.');
    let parse_component = |component: Option<&str>| -> Result<u16, AuthModelPackageLoadError> {
        component
            .ok_or_else(|| AuthModelPackageLoadError::Parse {
                path: path.to_path_buf(),
                message: format!("invalid package version `{value}`"),
            })?
            .parse::<u16>()
            .map_err(|error| AuthModelPackageLoadError::Parse {
                path: path.to_path_buf(),
                message: format!("invalid package version `{value}`: {error}"),
            })
    };

    let major = parse_component(components.next())?;
    let minor = parse_component(components.next())?;
    let patch = parse_component(components.next())?;
    if components.next().is_some() {
        return Err(AuthModelPackageLoadError::Parse {
            path: path.to_path_buf(),
            message: format!("invalid package version `{value}`"),
        });
    }
    Ok(PackageVersion::new(major, minor, patch))
}

fn parse_package_mode(value: &str, path: &Path) -> Result<PackageMode, AuthModelPackageLoadError> {
    match value {
        "replace" => Ok(PackageMode::Replace),
        "extend" => Ok(PackageMode::Extend),
        other => Err(AuthModelPackageLoadError::Parse {
            path: path.to_path_buf(),
            message: format!("unsupported auth package mode `{other}`"),
        }),
    }
}

fn load_capability_bindings(
    path: &Path,
    package: &str,
) -> Result<HashMap<Capability, CapabilityBinding>, AuthModelPackageLoadError> {
    if !path.is_file() {
        return Ok(HashMap::new());
    }

    let document = read_toml::<CapabilityBindingsDocument>(path)?;
    let mut bindings = HashMap::with_capacity(document.bindings.len());
    for (capability_name, binding) in document.bindings {
        let capability = Capability::from_str(&capability_name).ok_or_else(|| {
            AuthModelPackageLoadError::UnknownCapability {
                package: package.to_string(),
                capability: capability_name.clone(),
            }
        })?;
        let namespace = Namespace::from_str(&binding.resource_type).ok_or_else(|| {
            AuthModelPackageLoadError::UnknownNamespace {
                package: package.to_string(),
                namespace: binding.resource_type.clone(),
            }
        })?;
        let relation = Relation::from_str(&binding.permission).ok_or_else(|| {
            AuthModelPackageLoadError::UnknownRelation {
                package: package.to_string(),
                relation: binding.permission.clone(),
            }
        })?;

        bindings.insert(
            capability,
            CapabilityBinding {
                capability,
                resource_namespaces: vec![namespace],
                relation,
            },
        );
    }

    Ok(bindings)
}

fn load_model_schema(
    path: &Path,
    package: &str,
    base_schema: Option<&Schema>,
    replacement_mode: bool,
) -> Result<Schema, AuthModelPackageLoadError> {
    if !path.is_file() {
        return Ok(base_schema.cloned().unwrap_or_default());
    }

    let input = fs::read_to_string(path).map_err(|error| AuthModelPackageLoadError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;

    let mut schema = base_schema.cloned().unwrap_or_default();
    let mut current_namespace = None;
    let mut section = None;

    for (index, raw_line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("--") {
            continue;
        }
        if let Some(namespace_name) = line.strip_prefix("type ") {
            let namespace_name = namespace_name.trim();
            let namespace = Namespace::from_str(namespace_name).ok_or_else(|| {
                AuthModelPackageLoadError::UnknownNamespace {
                    package: package.to_string(),
                    namespace: namespace_name.to_string(),
                }
            })?;
            if !replacement_mode && !schema.namespaces.contains_key(namespace.as_str()) {
                return Err(AuthModelPackageLoadError::UnsupportedModelSyntax {
                    path: path.to_path_buf(),
                    line: line_number,
                    message: format!(
                        "extend-mode packages may only refine known namespaces; `{namespace_name}` is not available"
                    ),
                });
            }
            schema
                .namespaces
                .entry(namespace.as_str().to_string())
                .or_insert_with(NamespaceConfig::default);
            current_namespace = Some(namespace);
            section = None;
            continue;
        }
        if line == "relations" {
            section = Some(ModelSection::Relations);
            continue;
        }
        if line == "permissions" {
            section = Some(ModelSection::Permissions);
            continue;
        }

        let namespace =
            current_namespace.ok_or_else(|| AuthModelPackageLoadError::UnsupportedModelSyntax {
                path: path.to_path_buf(),
                line: line_number,
                message: "entries must appear inside a `type <namespace>` block".to_string(),
            })?;
        match section {
            Some(ModelSection::Relations) => {
                let relation_name = line
                    .split_once(':')
                    .map(|(name, _)| name.trim())
                    .ok_or_else(|| AuthModelPackageLoadError::UnsupportedModelSyntax {
                        path: path.to_path_buf(),
                        line: line_number,
                        message: "relation entries must use `<relation>: ...` syntax".to_string(),
                    })?;
                Relation::from_str(relation_name).ok_or_else(|| {
                    AuthModelPackageLoadError::UnknownRelation {
                        package: package.to_string(),
                        relation: relation_name.to_string(),
                    }
                })?;
                let namespace_rules = schema
                    .namespaces
                    .get_mut(namespace.as_str())
                    .expect("validated namespace exists in the schema");
                namespace_rules
                    .rules
                    .entry(relation_name.to_string())
                    .or_insert_with(Vec::new);
            }
            Some(ModelSection::Permissions) => {
                let (permission_name, source_name) = line.split_once('=').ok_or_else(|| {
                    AuthModelPackageLoadError::UnsupportedModelSyntax {
                        path: path.to_path_buf(),
                        line: line_number,
                        message: "permission entries must use `<permission> = <relation>` syntax"
                            .to_string(),
                    }
                })?;
                if source_name.contains('|') {
                    return Err(AuthModelPackageLoadError::UnsupportedModelSyntax {
                        path: path.to_path_buf(),
                        line: line_number,
                        message:
                            "multi-source permission expressions are not supported by the current file-backed loader"
                                .to_string(),
                    });
                }
                let permission = Relation::from_str(permission_name.trim()).ok_or_else(|| {
                    AuthModelPackageLoadError::UnknownRelation {
                        package: package.to_string(),
                        relation: permission_name.trim().to_string(),
                    }
                })?;
                let source = Relation::from_str(source_name.trim()).ok_or_else(|| {
                    AuthModelPackageLoadError::UnknownRelation {
                        package: package.to_string(),
                        relation: source_name.trim().to_string(),
                    }
                })?;
                let namespace_rules = schema
                    .namespaces
                    .get_mut(namespace.as_str())
                    .expect("validated namespace exists in the schema");
                namespace_rules.rules.insert(
                    permission.as_str().to_string(),
                    vec![RelationRule::Inherit(source.as_str().into())],
                );
            }
            None => {
                return Err(AuthModelPackageLoadError::UnsupportedModelSyntax {
                    path: path.to_path_buf(),
                    line: line_number,
                    message: "entries must appear under `relations` or `permissions`".to_string(),
                });
            }
        }
    }

    Ok(schema)
}
