use std::collections::{HashMap, HashSet};

use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind as CoreAdminContributionKind,
    AdminNavigationSection as CoreAdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition as CoreBulkOperationDefinition,
    BulkOperationKind as CoreBulkOperationKind, ModuleManifest,
};
use davenda_wasm::ExtensionRegistry;

use crate::error::AdminModelError;
use crate::ids::{AdminResourceId, AdminWidgetId, AuditEntryId, ResourceKind, WorkflowId};
use crate::validation::{require_non_empty, validate_route};

mod descriptors;
mod shell;

pub use descriptors::*;
pub use shell::*;
