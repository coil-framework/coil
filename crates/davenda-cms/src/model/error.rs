use super::*;
use std::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmsModelError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    InvalidPath { field: &'static str, value: String },
    DataPlan { error: DataModelError },
    MissingLiveRevision { page_id: String },
    CannotScheduleInThePast { publish_at: u64, now: u64 },
    NavigationCycle { item_id: String },
    DuplicateNavigationItem { item_id: String },
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
        }
    }
}

impl Error for CmsModelError {}

impl From<DataModelError> for CmsModelError {
    fn from(error: DataModelError) -> Self {
        Self::DataPlan { error }
    }
}
