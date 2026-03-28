use crate::ObservabilityError;
use crate::validation::{BrandId, CohortId, CustomerAppId, FeatureFlagId, SiteId};
use coil_config::Environment;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagTarget {
    Environment(Environment),
    CustomerApp(CustomerAppId),
    Site(SiteId),
    Brand(BrandId),
    Cohort(CohortId),
}

impl fmt::Display for FlagTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment(environment) => write!(f, "environment:{environment:?}"),
            Self::CustomerApp(app) => write!(f, "customer_app:{app}"),
            Self::Site(site) => write!(f, "site:{site}"),
            Self::Brand(brand) => write!(f, "brand:{brand}"),
            Self::Cohort(cohort) => write!(f, "cohort:{cohort}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlagRule {
    pub target: FlagTarget,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlag {
    pub id: FeatureFlagId,
    pub default_enabled: bool,
    pub rules: Vec<FeatureFlagRule>,
}

impl FeatureFlag {
    pub fn new(id: impl Into<String>, default_enabled: bool) -> Result<Self, ObservabilityError> {
        Ok(Self {
            id: FeatureFlagId::new(id)?,
            default_enabled,
            rules: Vec::new(),
        })
    }

    pub fn with_rule(
        mut self,
        target: FlagTarget,
        enabled: bool,
    ) -> Result<Self, ObservabilityError> {
        if self.rules.iter().any(|rule| rule.target == target) {
            return Err(ObservabilityError::DuplicateFlagRule {
                flag: self.id.to_string(),
                scope: target.to_string(),
            });
        }

        self.rules.push(FeatureFlagRule { target, enabled });
        Ok(self)
    }

    pub fn enabled_for(&self, context: &FeatureFlagContext) -> bool {
        self.rules
            .iter()
            .filter(|rule| context.matches(&rule.target))
            .map(|rule| rule.enabled)
            .next_back()
            .unwrap_or(self.default_enabled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlagContext {
    pub environment: Environment,
    pub customer_app: Option<CustomerAppId>,
    pub site: Option<SiteId>,
    pub brand: Option<BrandId>,
    pub cohorts: BTreeSet<CohortId>,
}

impl FeatureFlagContext {
    pub fn matches(&self, target: &FlagTarget) -> bool {
        match target {
            FlagTarget::Environment(environment) => &self.environment == environment,
            FlagTarget::CustomerApp(app) => self.customer_app.as_ref() == Some(app),
            FlagTarget::Site(site) => self.site.as_ref() == Some(site),
            FlagTarget::Brand(brand) => self.brand.as_ref() == Some(brand),
            FlagTarget::Cohort(cohort) => self.cohorts.contains(cohort),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeatureFlagRegistry {
    flags: BTreeMap<FeatureFlagId, FeatureFlag>,
}

impl FeatureFlagRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, flag: FeatureFlag) -> Result<(), ObservabilityError> {
        if self.flags.insert(flag.id.clone(), flag.clone()).is_some() {
            return Err(ObservabilityError::DuplicateFlag {
                flag: flag.id.to_string(),
            });
        }

        Ok(())
    }

    pub fn get(&self, id: &FeatureFlagId) -> Option<&FeatureFlag> {
        self.flags.get(id)
    }
}
