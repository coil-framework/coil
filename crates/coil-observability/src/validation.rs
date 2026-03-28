use crate::ObservabilityError;
use std::fmt;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ObservabilityError> {
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

token_type!(MetricName, "metric_name");
token_type!(DimensionKey, "dimension_key");
token_type!(FeatureFlagId, "feature_flag_id");
token_type!(CustomerAppId, "customer_app_id");
token_type!(SiteId, "site_id");
token_type!(BrandId, "brand_id");
token_type!(CohortId, "cohort_id");

pub fn validate_token(field: &'static str, value: String) -> Result<String, ObservabilityError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ObservabilityError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(ObservabilityError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}
