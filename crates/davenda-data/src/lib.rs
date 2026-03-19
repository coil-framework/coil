use std::fmt;
use std::time::Duration;

use davenda_config::{DatabaseConfig, DatabaseDriver};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DataModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("page size must be greater than zero")]
    InvalidPageSize,
    #[error("database pool sizing is invalid: min={min_connections} max={max_connections}")]
    InvalidPoolSizing {
        min_connections: u16,
        max_connections: u16,
    },
    #[error("statement timeout must be greater than zero")]
    InvalidStatementTimeout,
    #[error("migration `{migration_id}` is duplicated for owner `{owner}`")]
    DuplicateMigration { owner: String, migration_id: String },
}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, DataModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(QueryField, "query_field");
token_type!(MigrationId, "migration_id");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySort {
    pub field: QueryField,
    pub direction: SortDirection,
}

impl QuerySort {
    pub fn ascending(field: impl Into<String>) -> Result<Self, DataModelError> {
        Ok(Self {
            field: QueryField::new(field)?,
            direction: SortDirection::Asc,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    Prefix,
    Range,
    In,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryFilter {
    pub field: QueryField,
    pub operator: FilterOperator,
    pub values: Vec<String>,
}

impl QueryFilter {
    pub fn new(
        field: impl Into<String>,
        operator: FilterOperator,
        values: Vec<String>,
    ) -> Result<Self, DataModelError> {
        Ok(Self {
            field: QueryField::new(field)?,
            operator,
            values: values
                .into_iter()
                .map(|value| require_non_empty("filter_value", value))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub number: u32,
    pub size: u16,
}

impl PageRequest {
    pub fn new(number: u32, size: u16) -> Result<Self, DataModelError> {
        if size == 0 {
            return Err(DataModelError::InvalidPageSize);
        }

        Ok(Self { number, size })
    }

    pub fn offset(&self) -> usize {
        self.number as usize * self.size as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationVisibility {
    PublishedOnly,
    IncludeDrafts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryCacheScope {
    Public,
    LocaleScoped,
    UserScoped,
    Uncacheable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryContext {
    pub locale: Option<String>,
    pub principal_id: Option<String>,
    pub publication_visibility: PublicationVisibility,
    pub cache_scope: QueryCacheScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySpec {
    pub filters: Vec<QueryFilter>,
    pub sort: Vec<QuerySort>,
    pub page: PageRequest,
    pub context: QueryContext,
}

impl QuerySpec {
    pub fn new(page: PageRequest, context: QueryContext) -> Self {
        Self {
            filters: Vec::new(),
            sort: Vec::new(),
            page,
            context,
        }
    }

    pub fn with_filter(mut self, filter: QueryFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn with_sort(mut self, sort: QuerySort) -> Self {
        self.sort.push(sort);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionIsolation {
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainWrite {
    pub resource: String,
    pub action: String,
}

impl DomainWrite {
    pub fn new(
        resource: impl Into<String>,
        action: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        Ok(Self {
            resource: require_non_empty("write_resource", resource.into())?,
            action: require_non_empty("write_action", action.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionPlan {
    pub name: String,
    pub isolation: TransactionIsolation,
    pub writes: Vec<DomainWrite>,
    pub after_commit_jobs: Vec<String>,
    pub after_commit_events: Vec<String>,
}

impl TransactionPlan {
    pub fn new(
        name: impl Into<String>,
        isolation: TransactionIsolation,
    ) -> Result<Self, DataModelError> {
        Ok(Self {
            name: require_non_empty("transaction_name", name.into())?,
            isolation,
            writes: Vec::new(),
            after_commit_jobs: Vec::new(),
            after_commit_events: Vec::new(),
        })
    }

    pub fn with_write(mut self, write: DomainWrite) -> Self {
        self.writes.push(write);
        self
    }

    pub fn with_after_commit_job(
        mut self,
        job_name: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        self.after_commit_jobs
            .push(require_non_empty("after_commit_job", job_name.into())?);
        Ok(self)
    }

    pub fn with_after_commit_event(
        mut self,
        event_name: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        self.after_commit_events
            .push(require_non_empty("after_commit_event", event_name.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationOwner {
    Core,
    Module(String),
    CustomerApp(String),
    AuthPackage(String),
}

impl fmt::Display for MigrationOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => f.write_str("core"),
            Self::Module(module) => write!(f, "module:{module}"),
            Self::CustomerApp(app) => write!(f, "customer_app:{app}"),
            Self::AuthPackage(package) => write!(f, "auth_package:{package}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStep {
    pub id: MigrationId,
    pub owner: MigrationOwner,
    pub order: u32,
    pub description: String,
    pub online_safe: bool,
}

impl MigrationStep {
    pub fn new(
        id: MigrationId,
        owner: MigrationOwner,
        order: u32,
        description: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        Ok(Self {
            id,
            owner,
            order,
            description: require_non_empty("migration_description", description.into())?,
            online_safe: true,
        })
    }

    pub fn blocking(mut self) -> Self {
        self.online_safe = false;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationPlan {
    steps: Vec<MigrationStep>,
}

impl MigrationPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, step: MigrationStep) -> Result<(), DataModelError> {
        if self
            .steps
            .iter()
            .any(|existing| existing.owner == step.owner && existing.id == step.id)
        {
            return Err(DataModelError::DuplicateMigration {
                owner: step.owner.to_string(),
                migration_id: step.id.to_string(),
            });
        }

        self.steps.push(step);
        self.steps.sort_by(|left, right| {
            owner_rank(&left.owner)
                .cmp(&owner_rank(&right.owner))
                .then(left.order.cmp(&right.order))
                .then(left.id.as_str().cmp(right.id.as_str()))
        });
        Ok(())
    }

    pub fn ordered_steps(&self) -> &[MigrationStep] {
        &self.steps
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPoolProfile {
    pub min_connections: u16,
    pub max_connections: u16,
    pub statement_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRuntime {
    pub driver: DatabaseDriver,
    pub connection_secret: Option<String>,
    pub schema: String,
    pub migrations_table: String,
    pub pool: ConnectionPoolProfile,
}

impl DataRuntime {
    pub fn from_config(config: &DatabaseConfig) -> Result<Self, DataModelError> {
        if config.max_connections == 0 || config.min_connections > config.max_connections {
            return Err(DataModelError::InvalidPoolSizing {
                min_connections: config.min_connections,
                max_connections: config.max_connections,
            });
        }

        if config.statement_timeout_secs == 0 {
            return Err(DataModelError::InvalidStatementTimeout);
        }

        Ok(Self {
            driver: config.driver,
            connection_secret: config.url.as_ref().map(|secret| secret.redacted()),
            schema: require_non_empty("database_schema", config.schema.clone())?,
            migrations_table: require_non_empty(
                "database_migrations_table",
                config.migrations_table.clone(),
            )?,
            pool: ConnectionPoolProfile {
                min_connections: config.min_connections,
                max_connections: config.max_connections,
                statement_timeout: Duration::from_secs(config.statement_timeout_secs),
            },
        })
    }
}

fn owner_rank(owner: &MigrationOwner) -> u8 {
    match owner {
        MigrationOwner::Core => 0,
        MigrationOwner::Module(_) => 1,
        MigrationOwner::AuthPackage(_) => 2,
        MigrationOwner::CustomerApp(_) => 3,
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, DataModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DataModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(DataModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, DataModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(DataModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_config::{DatabaseConfig, SecretRef};

    #[test]
    fn data_runtime_maps_database_config_into_pool_profile() {
        let runtime = DataRuntime::from_config(&DatabaseConfig {
            driver: DatabaseDriver::Postgres,
            url: Some(SecretRef::Env {
                var: "DATABASE_URL".to_string(),
            }),
            schema: "davenda".to_string(),
            migrations_table: "_migrations".to_string(),
            min_connections: 2,
            max_connections: 16,
            statement_timeout_secs: 15,
        })
        .unwrap();

        assert_eq!(runtime.driver, DatabaseDriver::Postgres);
        assert_eq!(
            runtime.connection_secret.as_deref(),
            Some("env:DATABASE_URL")
        );
        assert_eq!(runtime.schema, "davenda");
        assert_eq!(runtime.pool.max_connections, 16);
        assert_eq!(runtime.pool.statement_timeout, Duration::from_secs(15));
    }

    #[test]
    fn migration_plan_orders_steps_by_owner_and_order() {
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("001_core").unwrap(),
                MigrationOwner::Core,
                1,
                "core tables",
            )
            .unwrap(),
        )
        .unwrap();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("010_events").unwrap(),
                MigrationOwner::Module("events".to_string()),
                10,
                "events tables",
            )
            .unwrap(),
        )
        .unwrap();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("900_customer").unwrap(),
                MigrationOwner::CustomerApp("showcase".to_string()),
                900,
                "customer app projection",
            )
            .unwrap(),
        )
        .unwrap();

        let owners = plan
            .ordered_steps()
            .iter()
            .map(|step| step.owner.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            owners,
            vec![
                "core".to_string(),
                "module:events".to_string(),
                "customer_app:showcase".to_string(),
            ]
        );
    }

    #[test]
    fn duplicate_migrations_are_rejected_per_owner() {
        let mut plan = MigrationPlan::new();
        let step = MigrationStep::new(
            MigrationId::new("001_core").unwrap(),
            MigrationOwner::Core,
            1,
            "core tables",
        )
        .unwrap();
        plan.insert(step.clone()).unwrap();

        let error = plan.insert(step).unwrap_err();
        assert_eq!(
            error,
            DataModelError::DuplicateMigration {
                owner: "core".to_string(),
                migration_id: "001_core".to_string(),
            }
        );
    }

    #[test]
    fn query_specs_capture_filters_sorts_and_context() {
        let query = QuerySpec::new(
            PageRequest::new(1, 25).unwrap(),
            QueryContext {
                locale: Some("fr-FR".to_string()),
                principal_id: Some("user-42".to_string()),
                publication_visibility: PublicationVisibility::PublishedOnly,
                cache_scope: QueryCacheScope::UserScoped,
            },
        )
        .with_filter(
            QueryFilter::new(
                "event_slug",
                FilterOperator::Eq,
                vec!["spring-tasting".to_string()],
            )
            .unwrap(),
        )
        .with_sort(QuerySort::ascending("starts_at").unwrap());

        assert_eq!(query.page.offset(), 25);
        assert_eq!(query.filters.len(), 1);
        assert_eq!(query.sort[0].field.as_str(), "starts_at");
        assert_eq!(query.context.locale.as_deref(), Some("fr-FR"));
    }

    #[test]
    fn transaction_plans_keep_writes_separate_from_after_commit_work() {
        let plan = TransactionPlan::new("booking.create", TransactionIsolation::Serializable)
            .unwrap()
            .with_write(DomainWrite::new("booking", "insert").unwrap())
            .with_write(DomainWrite::new("capacity", "decrement").unwrap())
            .with_after_commit_job("send-booking-email")
            .unwrap()
            .with_after_commit_event("booking.created")
            .unwrap();

        assert_eq!(plan.writes.len(), 2);
        assert_eq!(
            plan.after_commit_jobs,
            vec!["send-booking-email".to_string()]
        );
        assert_eq!(
            plan.after_commit_events,
            vec!["booking.created".to_string()]
        );
    }
}
