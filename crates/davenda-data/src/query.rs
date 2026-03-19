use std::fmt;

use crate::{DataModelError, QueryField, require_non_empty};

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
