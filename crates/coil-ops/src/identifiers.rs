use crate::error::OpsModelError;
use crate::validation::validate_token;
use std::fmt;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, OpsModelError> {
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

token_type!(SearchIndexId, "search_index_id");
token_type!(SearchFieldId, "search_field_id");
token_type!(ReportId, "report_id");
token_type!(ReportExportId, "report_export_id");
token_type!(BulkOperationId, "bulk_operation_id");
token_type!(BulkExecutionId, "bulk_execution_id");
token_type!(RecoveryWorkflowId, "recovery_workflow_id");
token_type!(RecoveryExecutionId, "recovery_execution_id");
