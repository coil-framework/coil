use super::*;
use std::time::Duration;

use coil_auth::Capability;
use coil_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    DataRepositoryContribution, DataRepositoryQueryProfile,
};
use coil_data::{
    FilterOperator, MigrationId, MigrationOwner, MigrationPlan, MigrationStep, PageRequest,
    PublicationVisibility, QueryCacheScope, QueryContext, QueryField, QueryFilter, QuerySort,
    QuerySpec, RepositorySpec, TableName,
};
use coil_jobs::RetryPolicy;

mod core;
mod platform;
mod support;

pub use core::CmsModule;
