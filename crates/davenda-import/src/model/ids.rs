use super::*;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ImportModelError> {
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

token_type!(ImportRunId, "import_run_id");
token_type!(SourceSystemId, "source_system_id");
token_type!(ImporterId, "importer_id");
token_type!(SourceRecordKey, "source_record_key");
token_type!(TargetRecordId, "target_record_id");
token_type!(RollbackTriggerId, "rollback_trigger_id");
