use std::fmt;

use crate::DataModelError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueryField(String);

impl QueryField {
    pub fn new(value: impl Into<String>) -> Result<Self, DataModelError> {
        Ok(Self(validate_token("query_field", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for QueryField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MigrationId(String);

impl MigrationId {
    pub fn new(value: impl Into<String>) -> Result<Self, DataModelError> {
        Ok(Self(validate_token("migration_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MigrationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TableName(String);

impl TableName {
    pub fn new(value: impl Into<String>) -> Result<Self, DataModelError> {
        Ok(Self(validate_token("table_name", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TableName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, DataModelError> {
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

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, DataModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(DataModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
