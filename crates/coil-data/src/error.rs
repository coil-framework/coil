use coil_config::DatabaseDriver;
use thiserror::Error;

use crate::{FilterOperator, MutationAction};

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
