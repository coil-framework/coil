use std::fmt;

use crate::{
    CompiledStatement, DataModelError, DataValue, QueryField, QueryFilter, TableName,
    compile_filters, quote_identifier, render_placeholder, require_non_empty,
};

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
