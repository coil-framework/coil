use std::fmt;

use crate::error::AdminModelError;
use crate::validation::validate_token;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AdminModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(AdminResourceId, "admin_resource_id");
token_type!(AdminWidgetId, "admin_widget_id");
token_type!(WorkflowId, "workflow_id");
token_type!(AuditEntryId, "audit_entry_id");
token_type!(ResourceKind, "resource_kind");
