use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use coil_auth::AuthModelPackage;
use coil_config::PlatformConfig;
use coil_core::{
    AdminResourceContribution, BulkOperationDefinition, CapabilityValidationError,
    CoreServiceDependency, EventSubscription, JobContract, MigrationContract, ModuleDependency,
    ModuleDependencyKind, ModuleManifest, PlatformModule, ReportDefinition, RouteSurface,
    SearchIndexContribution, validate_module_capabilities,
};
use coil_data::{MigrationOwner, MigrationPlan};
use coil_i18n::LocaleTag;
use coil_report::{
    CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportModelError, ReportRow, ReportStatus,
};
use coil_runtime::{RuntimeBuildError, RuntimeBuilder, RuntimePlan};
use coil_template::TemplateNamespace;
use coil_wasm::{
    ContractVersion, ExtensionConfigValue, ExtensionInstallation, ExtensionPackage, WasmModelError,
};
use thiserror::Error;

mod composition;
mod doctor;
mod manifest;
mod migration;
mod types;
mod util;

pub use composition::*;
pub use doctor::*;
pub use manifest::*;
pub use migration::*;
pub use types::*;

pub(crate) use doctor::config_alignment_findings;
pub(crate) use migration::build_migration_summary;
pub(crate) use util::{
    difference, join_display, require_non_empty, sorted_locale_strings, sorted_strings,
    validate_hostname, validate_sha256, validate_token,
};

#[cfg(test)]
mod tests;
