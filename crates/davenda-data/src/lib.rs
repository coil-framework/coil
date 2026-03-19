use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use davenda_config::{DatabaseConfig, DatabaseDriver, SecretRef};
use sqlx::postgres::{PgArguments, PgConnectOptions, PgPoolOptions};
use sqlx::{Pool, Postgres};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
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
    #[error("repository `{repository}` must declare at least one projected field")]
    EmptyProjection { repository: String },
    #[error("field `{field}` is not declared on repository `{repository}`")]
    UnknownRepositoryField { repository: String, field: String },
    #[error("filter operator `{operator}` expected {expected} value(s) but received {actual}")]
    InvalidFilterArity {
        operator: FilterOperator,
        expected: &'static str,
        actual: usize,
    },
    #[error("transaction plan expected {expected} writes but received {actual} mutations")]
    TransactionWriteCountMismatch { expected: usize, actual: usize },
    #[error("mutation `{action}` on table `{table}` must declare at least one assignment")]
    MissingMutationAssignments {
        table: String,
        action: MutationAction,
    },
    #[error("mutation `{action}` on table `{table}` must declare at least one predicate")]
    MissingMutationPredicates {
        table: String,
        action: MutationAction,
    },
    #[error("upsert on table `{table}` must declare at least one conflict field")]
    MissingConflictFields { table: String },
    #[error("database connection secret is not configured")]
    MissingConnectionSecret,
    #[error("environment variable `{var}` is not set for the database connection secret")]
    MissingConnectionSecretEnv { var: String },
    #[error("secret reference `{secret_ref}` is not supported by the local data runtime")]
    UnsupportedSecretRef { secret_ref: String },
    #[error("database driver `{driver:?}` does not support sqlx-backed postgres execution")]
    UnsupportedSqlxDriver { driver: DatabaseDriver },
    #[error("database connection URL is invalid: {reason}")]
    InvalidConnectionUrl { reason: String },
    #[error("unsigned value `{value}` cannot be represented as a Postgres BIGINT bind")]
    UnsupportedUnsignedBindValue { value: u64 },
    #[error("sqlx execution failed: {reason}")]
    Sqlx { reason: String },
    #[error("migration `{migration_id}` has no SQL statements to apply")]
    MissingMigrationStatements { migration_id: String },
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
token_type!(TableName, "table_name");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl fmt::Display for SortDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asc => f.write_str("asc"),
            Self::Desc => f.write_str("desc"),
        }
    }
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

impl fmt::Display for FilterOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eq => f.write_str("eq"),
            Self::Prefix => f.write_str("prefix"),
            Self::Range => f.write_str("range"),
            Self::In => f.write_str("in"),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataValue {
    String(String),
    Int(i64),
    UInt(u64),
    Bool(bool),
}

impl From<&str> for DataValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for DataValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for DataValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<u64> for DataValue {
    fn from(value: u64) -> Self {
        Self::UInt(value)
    }
}

impl From<bool> for DataValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledStatement {
    pub sql: String,
    pub bind_values: Vec<DataValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledQuery {
    pub sql: String,
    pub bind_values: Vec<DataValue>,
    pub page: PageRequest,
    pub context: QueryContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepositoryContextBindings {
    pub locale_field: Option<QueryField>,
    pub publication_field: Option<QueryField>,
    pub published_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySpec {
    pub id: String,
    pub table: TableName,
    pub projection: Vec<QueryField>,
    pub filterable_fields: Vec<QueryField>,
    pub sortable_fields: Vec<QueryField>,
    pub default_sort: Vec<QuerySort>,
    pub context: RepositoryContextBindings,
}

impl RepositorySpec {
    pub fn new(
        id: impl Into<String>,
        table: TableName,
        projection: Vec<QueryField>,
    ) -> Result<Self, DataModelError> {
        let id = require_non_empty("repository_id", id.into())?;
        if projection.is_empty() {
            return Err(DataModelError::EmptyProjection { repository: id });
        }

        Ok(Self {
            id,
            table,
            filterable_fields: projection.clone(),
            sortable_fields: projection.clone(),
            projection,
            default_sort: Vec::new(),
            context: RepositoryContextBindings::default(),
        })
    }

    pub fn with_filterable_field(
        mut self,
        field: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        let field = QueryField::new(field)?;
        if !self.filterable_fields.contains(&field) {
            self.filterable_fields.push(field);
        }
        Ok(self)
    }

    pub fn with_sortable_field(mut self, field: impl Into<String>) -> Result<Self, DataModelError> {
        let field = QueryField::new(field)?;
        if !self.sortable_fields.contains(&field) {
            self.sortable_fields.push(field);
        }
        Ok(self)
    }

    pub fn with_default_sort(mut self, sort: QuerySort) -> Self {
        self.default_sort.push(sort);
        self
    }

    pub fn with_locale_field(mut self, field: impl Into<String>) -> Result<Self, DataModelError> {
        self.context.locale_field = Some(QueryField::new(field)?);
        Ok(self)
    }

    pub fn with_publication_field(
        mut self,
        field: impl Into<String>,
        published_value: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        self.context.publication_field = Some(QueryField::new(field)?);
        self.context.published_value = Some(require_non_empty(
            "published_value",
            published_value.into(),
        )?);
        Ok(self)
    }

    pub fn compile_query(&self, spec: &QuerySpec) -> Result<CompiledQuery, DataModelError> {
        let mut filters = Vec::new();
        if let (Some(locale), Some(locale_field)) = (
            spec.context.locale.as_ref(),
            self.context.locale_field.as_ref(),
        ) {
            filters.push(QueryFilter::new(
                locale_field.as_str(),
                FilterOperator::Eq,
                vec![locale.clone()],
            )?);
        }

        if let (PublicationVisibility::PublishedOnly, Some(publication_field), Some(published)) = (
            spec.context.publication_visibility,
            self.context.publication_field.as_ref(),
            self.context.published_value.as_ref(),
        ) {
            filters.push(QueryFilter::new(
                publication_field.as_str(),
                FilterOperator::Eq,
                vec![published.clone()],
            )?);
        }

        filters.extend(spec.filters.clone());

        for filter in &filters {
            ensure_repository_field(
                &self.id,
                &filter.field,
                &self.filterable_fields,
                self.context.locale_field.as_ref(),
                self.context.publication_field.as_ref(),
            )?;
        }

        let sort = if spec.sort.is_empty() {
            self.default_sort.clone()
        } else {
            spec.sort.clone()
        };

        for sort_field in &sort {
            ensure_repository_field(
                &self.id,
                &sort_field.field,
                &self.sortable_fields,
                self.context.locale_field.as_ref(),
                self.context.publication_field.as_ref(),
            )?;
        }

        let projection = self
            .projection
            .iter()
            .map(|field| quote_identifier(field.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut sql = format!(
            "SELECT {projection} FROM {}",
            quote_identifier(self.table.as_str())
        );

        let (where_clauses, bind_values, _) = compile_filters(&filters, 1)?;
        if !where_clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&where_clauses.join(" AND "));
        }

        if !sort.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(
                &sort
                    .iter()
                    .map(|sort| {
                        format!(
                            "{} {}",
                            quote_identifier(sort.field.as_str()),
                            sort.direction.to_string().to_uppercase()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }

        sql.push_str(&format!(
            " LIMIT {} OFFSET {}",
            spec.page.size,
            spec.page.offset()
        ));

        Ok(CompiledQuery {
            sql,
            bind_values,
            page: spec.page,
            context: spec.context.clone(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionIsolation {
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl fmt::Display for TransactionIsolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadCommitted => f.write_str("READ COMMITTED"),
            Self::RepeatableRead => f.write_str("REPEATABLE READ"),
            Self::Serializable => f.write_str("SERIALIZABLE"),
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationAction {
    Insert,
    Update,
    Upsert,
    Delete,
}

impl fmt::Display for MutationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Insert => f.write_str("insert"),
            Self::Update => f.write_str("update"),
            Self::Upsert => f.write_str("upsert"),
            Self::Delete => f.write_str("delete"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationField {
    pub field: QueryField,
    pub value: DataValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationSpec {
    pub table: TableName,
    pub action: MutationAction,
    pub predicates: Vec<QueryFilter>,
    pub assignments: Vec<MutationField>,
    pub conflict_fields: Vec<QueryField>,
}

impl MutationSpec {
    pub fn new(table: impl Into<String>, action: MutationAction) -> Result<Self, DataModelError> {
        Ok(Self {
            table: TableName::new(table)?,
            action,
            predicates: Vec::new(),
            assignments: Vec::new(),
            conflict_fields: Vec::new(),
        })
    }

    pub fn with_predicate(mut self, predicate: QueryFilter) -> Self {
        self.predicates.push(predicate);
        self
    }

    pub fn with_assignment(
        mut self,
        field: impl Into<String>,
        value: impl Into<DataValue>,
    ) -> Result<Self, DataModelError> {
        self.assignments.push(MutationField {
            field: QueryField::new(field)?,
            value: value.into(),
        });
        Ok(self)
    }

    pub fn on_conflict_field(mut self, field: impl Into<String>) -> Result<Self, DataModelError> {
        self.conflict_fields.push(QueryField::new(field)?);
        Ok(self)
    }

    pub fn compile(&self, start_index: usize) -> Result<CompiledStatement, DataModelError> {
        match self.action {
            MutationAction::Insert => self.compile_insert(start_index),
            MutationAction::Update => self.compile_update(start_index),
            MutationAction::Upsert => self.compile_upsert(start_index),
            MutationAction::Delete => self.compile_delete(start_index),
        }
    }

    fn compile_insert(&self, start_index: usize) -> Result<CompiledStatement, DataModelError> {
        if self.assignments.is_empty() {
            return Err(DataModelError::MissingMutationAssignments {
                table: self.table.to_string(),
                action: self.action,
            });
        }

        let columns = self
            .assignments
            .iter()
            .map(|assignment| quote_identifier(assignment.field.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let placeholders = (start_index..start_index + self.assignments.len())
            .map(render_placeholder)
            .collect::<Vec<_>>()
            .join(", ");

        Ok(CompiledStatement {
            sql: format!(
                "INSERT INTO {} ({columns}) VALUES ({placeholders})",
                quote_identifier(self.table.as_str())
            ),
            bind_values: self
                .assignments
                .iter()
                .map(|assignment| assignment.value.clone())
                .collect(),
        })
    }

    fn compile_update(&self, start_index: usize) -> Result<CompiledStatement, DataModelError> {
        if self.assignments.is_empty() {
            return Err(DataModelError::MissingMutationAssignments {
                table: self.table.to_string(),
                action: self.action,
            });
        }
        if self.predicates.is_empty() {
            return Err(DataModelError::MissingMutationPredicates {
                table: self.table.to_string(),
                action: self.action,
            });
        }

        let set_clause = self
            .assignments
            .iter()
            .enumerate()
            .map(|(offset, assignment)| {
                format!(
                    "{} = {}",
                    quote_identifier(assignment.field.as_str()),
                    render_placeholder(start_index + offset)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        let (where_clauses, mut bind_values, _) =
            compile_filters(&self.predicates, start_index + self.assignments.len())?;
        let mut assignment_values = self
            .assignments
            .iter()
            .map(|assignment| assignment.value.clone())
            .collect::<Vec<_>>();
        assignment_values.append(&mut bind_values);

        Ok(CompiledStatement {
            sql: format!(
                "UPDATE {} SET {set_clause} WHERE {}",
                quote_identifier(self.table.as_str()),
                where_clauses.join(" AND ")
            ),
            bind_values: assignment_values,
        })
    }

    fn compile_upsert(&self, start_index: usize) -> Result<CompiledStatement, DataModelError> {
        if self.assignments.is_empty() {
            return Err(DataModelError::MissingMutationAssignments {
                table: self.table.to_string(),
                action: self.action,
            });
        }
        if self.conflict_fields.is_empty() {
            return Err(DataModelError::MissingConflictFields {
                table: self.table.to_string(),
            });
        }

        let insert = self.compile_insert(start_index)?;
        let conflict = self
            .conflict_fields
            .iter()
            .map(|field| quote_identifier(field.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        let update_clause = self
            .assignments
            .iter()
            .map(|assignment| {
                format!(
                    "{} = EXCLUDED.{}",
                    quote_identifier(assignment.field.as_str()),
                    quote_identifier(assignment.field.as_str())
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        Ok(CompiledStatement {
            sql: format!(
                "{} ON CONFLICT ({conflict}) DO UPDATE SET {update_clause}",
                insert.sql
            ),
            bind_values: insert.bind_values,
        })
    }

    fn compile_delete(&self, start_index: usize) -> Result<CompiledStatement, DataModelError> {
        if self.predicates.is_empty() {
            return Err(DataModelError::MissingMutationPredicates {
                table: self.table.to_string(),
                action: self.action,
            });
        }

        let (where_clauses, bind_values, _) = compile_filters(&self.predicates, start_index)?;
        Ok(CompiledStatement {
            sql: format!(
                "DELETE FROM {} WHERE {}",
                quote_identifier(self.table.as_str()),
                where_clauses.join(" AND ")
            ),
            bind_values,
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
pub struct CompiledTransaction {
    pub begin_sql: String,
    pub commit_sql: String,
    pub statements: Vec<CompiledStatement>,
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
    pub statements: Vec<String>,
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
            statements: Vec::new(),
        })
    }

    pub fn blocking(mut self) -> Self {
        self.online_safe = false;
        self
    }

    pub fn with_statement(mut self, sql: impl Into<String>) -> Result<Self, DataModelError> {
        self.statements
            .push(require_non_empty("migration_statement", sql.into())?);
        Ok(self)
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationRegistry {
    plan: MigrationPlan,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plan: &MigrationPlan) -> Result<(), DataModelError> {
        for step in plan.ordered_steps() {
            self.plan.insert(step.clone())?;
        }
        Ok(())
    }

    pub fn composed_plan(&self) -> &MigrationPlan {
        &self.plan
    }

    pub fn compile_apply_batch(
        &self,
        runtime: &DataRuntime,
    ) -> Result<CompiledMigrationBatch, DataModelError> {
        let mut statements = Vec::new();
        let migrations_table =
            quote_identifier(&format!("{}.{}", runtime.schema, runtime.migrations_table));
        statements.push(CompiledStatement {
            sql: format!(
                "CREATE TABLE IF NOT EXISTS {migrations_table} (owner TEXT NOT NULL, migration_id TEXT NOT NULL, description TEXT NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), PRIMARY KEY (owner, migration_id))"
            ),
            bind_values: Vec::new(),
        });

        for step in self.plan.ordered_steps() {
            if step.statements.is_empty() {
                return Err(DataModelError::MissingMigrationStatements {
                    migration_id: step.id.to_string(),
                });
            }

            for sql in &step.statements {
                statements.push(CompiledStatement {
                    sql: sql.clone(),
                    bind_values: Vec::new(),
                });
            }

            statements.push(CompiledStatement {
                sql: format!(
                    "INSERT INTO {migrations_table} (owner, migration_id, description) VALUES ($1, $2, $3) ON CONFLICT (owner, migration_id) DO NOTHING"
                ),
                bind_values: vec![
                    DataValue::String(step.owner.to_string()),
                    DataValue::String(step.id.to_string()),
                    DataValue::String(step.description.clone()),
                ],
            });
        }

        Ok(CompiledMigrationBatch { statements })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledMigrationBatch {
    pub statements: Vec<CompiledStatement>,
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
    pub connection_secret_ref: Option<SecretRef>,
    pub connection_secret: Option<String>,
    pub schema: String,
    pub migrations_table: String,
    pub pool: ConnectionPoolProfile,
}

#[derive(Debug, Clone)]
pub struct PostgresDataClient {
    pub runtime: DataRuntime,
    pub connection_url: String,
    pub pool: Pool<Postgres>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementExecution {
    pub rows_affected: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionExecution {
    pub statements_executed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationBatchExecution {
    pub statements_executed: usize,
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
            connection_secret_ref: config.url.clone(),
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

    pub fn resolve_connection_url(&self) -> Result<String, DataModelError> {
        match self.connection_secret_ref.as_ref() {
            Some(SecretRef::Env { var }) => std::env::var(var)
                .map_err(|_| DataModelError::MissingConnectionSecretEnv { var: var.clone() }),
            Some(secret_ref) => Err(DataModelError::UnsupportedSecretRef {
                secret_ref: secret_ref.redacted(),
            }),
            None => Err(DataModelError::MissingConnectionSecret),
        }
    }

    pub fn connect_lazy_postgres(&self) -> Result<PostgresDataClient, DataModelError> {
        if self.driver != DatabaseDriver::Postgres {
            return Err(DataModelError::UnsupportedSqlxDriver {
                driver: self.driver,
            });
        }

        let connection_url = self.resolve_connection_url()?;
        let options = PgConnectOptions::from_str(&connection_url).map_err(|error| {
            DataModelError::InvalidConnectionUrl {
                reason: error.to_string(),
            }
        })?;
        let pool = PgPoolOptions::new()
            .min_connections(u32::from(self.pool.min_connections))
            .max_connections(u32::from(self.pool.max_connections))
            .acquire_timeout(self.pool.statement_timeout)
            .connect_lazy_with(options);

        Ok(PostgresDataClient {
            runtime: self.clone(),
            connection_url,
            pool,
        })
    }

    pub fn compile_query(
        &self,
        repository: &RepositorySpec,
        spec: &QuerySpec,
    ) -> Result<CompiledQuery, DataModelError> {
        repository.compile_query(spec)
    }

    pub fn compile_transaction(
        &self,
        plan: &TransactionPlan,
        mutations: &[MutationSpec],
    ) -> Result<CompiledTransaction, DataModelError> {
        if plan.writes.len() != mutations.len() {
            return Err(DataModelError::TransactionWriteCountMismatch {
                expected: plan.writes.len(),
                actual: mutations.len(),
            });
        }

        let mut statements = Vec::new();
        let mut bind_index = 1;
        for mutation in mutations {
            let compiled = mutation.compile(bind_index)?;
            bind_index += compiled.bind_values.len();
            statements.push(compiled);
        }

        let jobs_table = quote_identifier(&format!("{}.job_outbox", self.schema));
        for job in &plan.after_commit_jobs {
            statements.push(CompiledStatement {
                sql: format!(
                    "INSERT INTO {jobs_table} (transaction_name, job_name) VALUES ($1, $2)"
                ),
                bind_values: vec![
                    DataValue::String(plan.name.clone()),
                    DataValue::String(job.clone()),
                ],
            });
        }

        let events_table = quote_identifier(&format!("{}.event_outbox", self.schema));
        for event in &plan.after_commit_events {
            statements.push(CompiledStatement {
                sql: format!(
                    "INSERT INTO {events_table} (transaction_name, event_name) VALUES ($1, $2)"
                ),
                bind_values: vec![
                    DataValue::String(plan.name.clone()),
                    DataValue::String(event.clone()),
                ],
            });
        }

        Ok(CompiledTransaction {
            begin_sql: "BEGIN".to_string(),
            commit_sql: "COMMIT".to_string(),
            statements: std::iter::once(CompiledStatement {
                sql: format!("SET TRANSACTION ISOLATION LEVEL {}", plan.isolation),
                bind_values: Vec::new(),
            })
            .chain(statements)
            .collect(),
        })
    }

    pub fn compile_migrations(
        &self,
        registry: &MigrationRegistry,
    ) -> Result<CompiledMigrationBatch, DataModelError> {
        registry.compile_apply_batch(self)
    }
}

impl PostgresDataClient {
    pub async fn ping(&self) -> Result<(), DataModelError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;
        Ok(())
    }

    pub async fn execute_statement(
        &self,
        statement: &CompiledStatement,
    ) -> Result<StatementExecution, DataModelError> {
        self.apply_statement_timeout().await?;
        let result = bind_query(sqlx::query(&statement.sql), &statement.bind_values)?
            .execute(&self.pool)
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;

        Ok(StatementExecution {
            rows_affected: result.rows_affected(),
        })
    }

    pub async fn execute_transaction(
        &self,
        transaction: &CompiledTransaction,
    ) -> Result<TransactionExecution, DataModelError> {
        let mut tx = self.pool.begin().await.map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        sqlx::query(&format!(
            "SET LOCAL statement_timeout = {}",
            self.runtime.pool.statement_timeout.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        for statement in &transaction.statements {
            bind_query(sqlx::query(&statement.sql), &statement.bind_values)?
                .execute(&mut *tx)
                .await
                .map_err(|error| DataModelError::Sqlx {
                    reason: error.to_string(),
                })?;
        }

        tx.commit().await.map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        Ok(TransactionExecution {
            statements_executed: transaction.statements.len(),
        })
    }

    pub async fn apply_migrations(
        &self,
        batch: &CompiledMigrationBatch,
    ) -> Result<MigrationBatchExecution, DataModelError> {
        let mut tx = self.pool.begin().await.map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        sqlx::query(&format!(
            "SET LOCAL statement_timeout = {}",
            self.runtime.pool.statement_timeout.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        for statement in &batch.statements {
            bind_query(sqlx::query(&statement.sql), &statement.bind_values)?
                .execute(&mut *tx)
                .await
                .map_err(|error| DataModelError::Sqlx {
                    reason: error.to_string(),
                })?;
        }

        tx.commit().await.map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        Ok(MigrationBatchExecution {
            statements_executed: batch.statements.len(),
        })
    }

    async fn apply_statement_timeout(&self) -> Result<(), DataModelError> {
        sqlx::query(&format!(
            "SET statement_timeout = {}",
            self.runtime.pool.statement_timeout.as_millis()
        ))
        .execute(&self.pool)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;
        Ok(())
    }
}

fn bind_query<'q>(
    mut query: sqlx::query::Query<'q, Postgres, PgArguments>,
    values: &[DataValue],
) -> Result<sqlx::query::Query<'q, Postgres, PgArguments>, DataModelError> {
    for value in values {
        query = match value {
            DataValue::String(value) => query.bind(value.clone()),
            DataValue::Int(value) => query.bind(*value),
            DataValue::UInt(value) => {
                let value =
                    i64::try_from(*value).map_err(|_| DataModelError::UnsupportedUnsignedBindValue {
                        value: *value,
                    })?;
                query.bind(value)
            }
            DataValue::Bool(value) => query.bind(*value),
        };
    }

    Ok(query)
}

fn owner_rank(owner: &MigrationOwner) -> u8 {
    match owner {
        MigrationOwner::Core => 0,
        MigrationOwner::Module(_) => 1,
        MigrationOwner::AuthPackage(_) => 2,
        MigrationOwner::CustomerApp(_) => 3,
    }
}

fn ensure_repository_field(
    repository: &str,
    field: &QueryField,
    allowed: &[QueryField],
    locale_field: Option<&QueryField>,
    publication_field: Option<&QueryField>,
) -> Result<(), DataModelError> {
    if allowed.contains(field)
        || locale_field.is_some_and(|allowed_field| allowed_field == field)
        || publication_field.is_some_and(|allowed_field| allowed_field == field)
    {
        Ok(())
    } else {
        Err(DataModelError::UnknownRepositoryField {
            repository: repository.to_string(),
            field: field.to_string(),
        })
    }
}

fn compile_filters(
    filters: &[QueryFilter],
    start_index: usize,
) -> Result<(Vec<String>, Vec<DataValue>, usize), DataModelError> {
    let mut clauses = Vec::new();
    let mut bind_values = Vec::new();
    let mut index = start_index;

    for filter in filters {
        let (clause, values, next_index) = compile_filter(filter, index)?;
        clauses.push(clause);
        bind_values.extend(values);
        index = next_index;
    }

    Ok((clauses, bind_values, index))
}

fn compile_filter(
    filter: &QueryFilter,
    start_index: usize,
) -> Result<(String, Vec<DataValue>, usize), DataModelError> {
    let field = quote_identifier(filter.field.as_str());
    match filter.operator {
        FilterOperator::Eq => {
            ensure_filter_arity(filter, "exactly 1", 1..=1)?;
            Ok((
                format!("{field} = {}", render_placeholder(start_index)),
                vec![DataValue::String(filter.values[0].clone())],
                start_index + 1,
            ))
        }
        FilterOperator::Prefix => {
            ensure_filter_arity(filter, "exactly 1", 1..=1)?;
            Ok((
                format!("{field} LIKE {}", render_placeholder(start_index)),
                vec![DataValue::String(format!("{}%", filter.values[0]))],
                start_index + 1,
            ))
        }
        FilterOperator::Range => {
            ensure_filter_arity(filter, "exactly 2", 2..=2)?;
            Ok((
                format!(
                    "{field} BETWEEN {} AND {}",
                    render_placeholder(start_index),
                    render_placeholder(start_index + 1)
                ),
                vec![
                    DataValue::String(filter.values[0].clone()),
                    DataValue::String(filter.values[1].clone()),
                ],
                start_index + 2,
            ))
        }
        FilterOperator::In => {
            ensure_filter_arity(filter, "at least 1", 1..)?;
            let placeholders = (start_index..start_index + filter.values.len())
                .map(render_placeholder)
                .collect::<Vec<_>>()
                .join(", ");
            Ok((
                format!("{field} IN ({placeholders})"),
                filter
                    .values
                    .iter()
                    .cloned()
                    .map(DataValue::String)
                    .collect(),
                start_index + filter.values.len(),
            ))
        }
    }
}

fn ensure_filter_arity(
    filter: &QueryFilter,
    expected: &'static str,
    range: impl std::ops::RangeBounds<usize>,
) -> Result<(), DataModelError> {
    let actual = filter.values.len();
    let contains = match (range.start_bound(), range.end_bound()) {
        (std::ops::Bound::Included(start), std::ops::Bound::Included(end)) => {
            actual >= *start && actual <= *end
        }
        (std::ops::Bound::Included(start), std::ops::Bound::Unbounded) => actual >= *start,
        _ => false,
    };

    if contains {
        Ok(())
    } else {
        Err(DataModelError::InvalidFilterArity {
            operator: filter.operator,
            expected,
            actual,
        })
    }
}

fn quote_identifier(identifier: &str) -> String {
    identifier
        .split('.')
        .map(|part| format!("\"{}\"", part.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(".")
}

fn render_placeholder(index: usize) -> String {
    format!("${index}")
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
    use std::env;

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
            runtime.connection_secret_ref,
            Some(SecretRef::Env {
                var: "DATABASE_URL".to_string()
            })
        );
        assert_eq!(
            runtime.connection_secret.as_deref(),
            Some("env:DATABASE_URL")
        );
        assert_eq!(runtime.schema, "davenda");
        assert_eq!(runtime.pool.max_connections, 16);
        assert_eq!(runtime.pool.statement_timeout, Duration::from_secs(15));
    }

    #[test]
    fn runtime_can_create_a_lazy_postgres_client() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let var = "DAVENDA_TEST_DATABASE_URL";
            let previous = env::var(var).ok();
            unsafe {
                env::set_var(var, "postgres://davenda:davenda@localhost/davenda");
            }

            let runtime = DataRuntime::from_config(&DatabaseConfig {
                url: Some(SecretRef::Env {
                    var: var.to_string(),
                }),
                ..DatabaseConfig::default()
            })
            .unwrap();
            let client = runtime.connect_lazy_postgres().unwrap();

            assert_eq!(client.runtime.driver, DatabaseDriver::Postgres);
            assert_eq!(
                client.connection_url,
                "postgres://davenda:davenda@localhost/davenda"
            );

            match previous {
                Some(value) => unsafe {
                    env::set_var(var, value);
                },
                None => unsafe {
                    env::remove_var(var);
                },
            }
        });
    }

    #[test]
    fn connect_lazy_postgres_requires_a_resolvable_connection_secret() {
        let runtime = DataRuntime::from_config(&DatabaseConfig {
            url: None,
            ..DatabaseConfig::default()
        })
        .unwrap();

        assert_eq!(
            runtime.connect_lazy_postgres().unwrap_err(),
            DataModelError::MissingConnectionSecret
        );
    }

    #[test]
    fn bind_query_rejects_unsigned_values_that_exceed_bigint() {
        match bind_query(sqlx::query("SELECT $1"), &[DataValue::UInt(u64::MAX)]) {
            Err(error) => assert_eq!(
                error,
                DataModelError::UnsupportedUnsignedBindValue { value: u64::MAX }
            ),
            Ok(_) => panic!("expected oversized unsigned bind to be rejected"),
        }
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

    #[test]
    fn repository_specs_compile_locale_and_publication_aware_sql() {
        let repository = RepositorySpec::new(
            "cms.pages",
            TableName::new("davenda.cms_pages").unwrap(),
            vec![
                QueryField::new("page_id").unwrap(),
                QueryField::new("title").unwrap(),
                QueryField::new("live_path").unwrap(),
                QueryField::new("updated_at").unwrap(),
            ],
        )
        .unwrap()
        .with_locale_field("locale")
        .unwrap()
        .with_publication_field("workflow_status", "published")
        .unwrap()
        .with_filterable_field("slug")
        .unwrap()
        .with_default_sort(QuerySort::ascending("live_path").unwrap());

        let query = QuerySpec::new(
            PageRequest::new(0, 20).unwrap(),
            QueryContext {
                locale: Some("fr-FR".to_string()),
                principal_id: None,
                publication_visibility: PublicationVisibility::PublishedOnly,
                cache_scope: QueryCacheScope::LocaleScoped,
            },
        )
        .with_filter(
            QueryFilter::new("slug", FilterOperator::Eq, vec!["home".to_string()]).unwrap(),
        );

        let compiled = repository.compile_query(&query).unwrap();
        assert_eq!(
            compiled.sql,
            "SELECT \"page_id\", \"title\", \"live_path\", \"updated_at\" FROM \"davenda\".\"cms_pages\" WHERE \"locale\" = $1 AND \"workflow_status\" = $2 AND \"slug\" = $3 ORDER BY \"live_path\" ASC LIMIT 20 OFFSET 0"
        );
        assert_eq!(
            compiled.bind_values,
            vec![
                DataValue::String("fr-FR".to_string()),
                DataValue::String("published".to_string()),
                DataValue::String("home".to_string()),
            ]
        );
    }

    #[test]
    fn runtime_compiles_mutations_and_outbox_delivery_sql() {
        let runtime = DataRuntime::from_config(&DatabaseConfig::default()).unwrap();
        let plan =
            TransactionPlan::new("events.booking.confirm", TransactionIsolation::Serializable)
                .unwrap()
                .with_write(DomainWrite::new("events.bookings", "update").unwrap())
                .with_write(DomainWrite::new("events.reservations", "delete").unwrap())
                .with_after_commit_job("events.jobs.notifications.booking_confirmed")
                .unwrap()
                .with_after_commit_event("events.booking.confirmed")
                .unwrap();
        let mutations = vec![
            MutationSpec::new("davenda.events_bookings", MutationAction::Update)
                .unwrap()
                .with_assignment("status", "confirmed")
                .unwrap()
                .with_predicate(
                    QueryFilter::new(
                        "booking_id",
                        FilterOperator::Eq,
                        vec!["booking-1".to_string()],
                    )
                    .unwrap(),
                ),
            MutationSpec::new("davenda.events_reservations", MutationAction::Delete)
                .unwrap()
                .with_predicate(
                    QueryFilter::new(
                        "reservation_id",
                        FilterOperator::Eq,
                        vec!["reservation-1".to_string()],
                    )
                    .unwrap(),
                ),
        ];

        let compiled = runtime.compile_transaction(&plan, &mutations).unwrap();
        assert_eq!(compiled.begin_sql, "BEGIN");
        assert_eq!(compiled.commit_sql, "COMMIT");
        assert_eq!(
            compiled.statements[0].sql,
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"
        );
        assert_eq!(
            compiled.statements[1].sql,
            "UPDATE \"davenda\".\"events_bookings\" SET \"status\" = $1 WHERE \"booking_id\" = $2"
        );
        assert_eq!(
            compiled.statements[2].sql,
            "DELETE FROM \"davenda\".\"events_reservations\" WHERE \"reservation_id\" = $3"
        );
        assert!(
            compiled
                .statements
                .iter()
                .any(|statement| statement.sql.contains("\"public\".\"job_outbox\""))
        );
        assert!(
            compiled
                .statements
                .iter()
                .any(|statement| statement.sql.contains("\"public\".\"event_outbox\""))
        );
    }

    #[test]
    fn migration_registry_compiles_apply_batch_with_ledger_entries() {
        let runtime = DataRuntime::from_config(&DatabaseConfig::default()).unwrap();
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("001_pages").unwrap(),
                MigrationOwner::Module("cms".to_string()),
                10,
                "create cms pages table",
            )
            .unwrap()
            .with_statement("CREATE TABLE davenda.cms_pages (page_id TEXT PRIMARY KEY)")
            .unwrap(),
        )
        .unwrap();
        let mut registry = MigrationRegistry::new();
        registry.register(&plan).unwrap();

        let batch = registry.compile_apply_batch(&runtime).unwrap();
        assert!(
            batch.statements[0]
                .sql
                .contains("\"public\".\"_davenda_migrations\"")
        );
        assert_eq!(
            batch.statements[1].sql,
            "CREATE TABLE davenda.cms_pages (page_id TEXT PRIMARY KEY)"
        );
        assert!(batch.statements[2].sql.contains("ON CONFLICT"));
        assert_eq!(
            batch.statements[2].bind_values,
            vec![
                DataValue::String("module:cms".to_string()),
                DataValue::String("001_pages".to_string()),
                DataValue::String("create cms pages table".to_string()),
            ]
        );
    }
}
