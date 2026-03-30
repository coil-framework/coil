use super::*;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmsModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidPath {
        field: &'static str,
        value: String,
    },
    DataPlan {
        error: DataModelError,
    },
    MissingLiveRevision {
        page_id: String,
    },
    CannotScheduleInThePast {
        publish_at: u64,
        now: u64,
    },
    NavigationCycle {
        item_id: String,
    },
    DuplicateNavigationItem {
        item_id: String,
    },
    DuplicateBlockFieldSchema {
        block_type_id: String,
        field_id: String,
    },
    UnknownBlockField {
        block_type_id: String,
        field_id: String,
    },
    MissingRequiredBlockField {
        block_type_id: String,
        field_id: String,
    },
    EmptyBlockFieldValues {
        block_type_id: String,
        field_id: String,
    },
    BlockFieldDoesNotAllowMultiple {
        block_type_id: String,
        field_id: String,
    },
    InvalidBlockFieldValueKind {
        block_type_id: String,
        field_id: String,
        expected: BlockFieldValueKind,
        actual: BlockFieldValueKind,
    },
    BlockTypeMismatch {
        expected_block_type_id: String,
        actual_block_type_id: String,
    },
    DuplicatePageBlockInstance {
        instance_id: String,
    },
}

impl fmt::Display for CmsModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidPath { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DataPlan { error } => write!(f, "{error}"),
            Self::MissingLiveRevision { page_id } => {
                write!(f, "page `{page_id}` has no live revision")
            }
            Self::CannotScheduleInThePast { publish_at, now } => write!(
                f,
                "scheduled publish time `{publish_at}` must be greater than current time `{now}`"
            ),
            Self::NavigationCycle { item_id } => {
                write!(f, "navigation item `{item_id}` introduces a cycle")
            }
            Self::DuplicateNavigationItem { item_id } => {
                write!(f, "navigation item `{item_id}` is duplicated in the tree")
            }
            Self::DuplicateBlockFieldSchema {
                block_type_id,
                field_id,
            } => write!(
                f,
                "block type `{block_type_id}` defines field `{field_id}` more than once"
            ),
            Self::UnknownBlockField {
                block_type_id,
                field_id,
            } => write!(
                f,
                "block field `{field_id}` is not defined on block type `{block_type_id}`"
            ),
            Self::MissingRequiredBlockField {
                block_type_id,
                field_id,
            } => write!(
                f,
                "required block field `{field_id}` is missing for block type `{block_type_id}`"
            ),
            Self::EmptyBlockFieldValues {
                block_type_id,
                field_id,
            } => write!(
                f,
                "block field `{field_id}` on block type `{block_type_id}` must have at least one value"
            ),
            Self::BlockFieldDoesNotAllowMultiple {
                block_type_id,
                field_id,
            } => write!(
                f,
                "block field `{field_id}` on block type `{block_type_id}` does not allow multiple values"
            ),
            Self::InvalidBlockFieldValueKind {
                block_type_id,
                field_id,
                expected,
                actual,
            } => write!(
                f,
                "block field `{field_id}` on block type `{block_type_id}` expects `{expected}` but got `{actual}`"
            ),
            Self::BlockTypeMismatch {
                expected_block_type_id,
                actual_block_type_id,
            } => write!(
                f,
                "block instance expects schema `{expected_block_type_id}` but contains `{actual_block_type_id}`"
            ),
            Self::DuplicatePageBlockInstance { instance_id } => {
                write!(
                    f,
                    "page block instance `{instance_id}` is duplicated in the revision"
                )
            }
        }
    }
}

impl Error for CmsModelError {}

impl From<DataModelError> for CmsModelError {
    fn from(error: DataModelError) -> Self {
        Self::DataPlan { error }
    }
}
