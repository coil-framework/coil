use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition as ManifestBulkOperationDefinition,
    BulkOperationKind as ManifestBulkOperationKind,
    BulkOperationScope as ManifestBulkOperationScope, CapabilityContract, CoreServiceDependency,
    EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind, HttpSurfaceArea,
    HttpSurfaceContribution, HttpSurfaceMethod, IntegrationKind, IntegrationPoint, JobContract,
    JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest,
    PlatformModule, RegistrationError, ReportDefinition as ManifestReportDefinition,
    ReportDeliveryMode as ManifestReportDeliveryMode, ReportFormat as ManifestReportFormat,
    ReportSensitivity as ManifestReportSensitivity, RouteSurface, RouteSurfaceKind,
    SearchDocumentKind as ManifestSearchDocumentKind,
    SearchFieldContribution as ManifestSearchFieldContribution,
    SearchFieldRole as ManifestSearchFieldRole,
    SearchIndexContribution as ManifestSearchIndexContribution,
    SearchInvalidationRule as ManifestSearchInvalidationRule,
    SearchInvalidationTrigger as ManifestSearchInvalidationTrigger,
    SearchRebuildStrategy as ManifestSearchRebuildStrategy,
    SearchVisibility as ManifestSearchVisibility, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};
use davenda_jobs::{
    IdempotencyKey, JobId, JobInstant, JobName, JobSpec, JobsModelError, JobsPlanner, JobsRuntime,
    PlannedJob, RetryPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpsModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateIdentifier {
        kind: &'static str,
        id: String,
    },
    DuplicateField {
        index_id: String,
        field_id: String,
    },
    MissingCapability {
        operation: &'static str,
        required: Capability,
    },
    InvalidSearchVisibility {
        index_id: String,
        reason: String,
    },
    InvalidReportDelivery {
        report_id: String,
        reason: String,
    },
    InvalidBulkOperation {
        operation_id: String,
        reason: String,
    },
    InvalidItemCount {
        operation: &'static str,
        count: usize,
    },
    JobsPlan {
        error: JobsModelError,
    },
}

impl fmt::Display for OpsModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateIdentifier { kind, id } => write!(f, "{kind} `{id}` is duplicated"),
            Self::DuplicateField { index_id, field_id } => {
                write!(f, "search index `{index_id}` duplicates field `{field_id}`")
            }
            Self::MissingCapability {
                operation,
                required,
            } => {
                write!(f, "{operation} requires capability `{required}`")
            }
            Self::InvalidSearchVisibility { index_id, reason } => {
                write!(
                    f,
                    "search index `{index_id}` has invalid visibility: {reason}"
                )
            }
            Self::InvalidReportDelivery { report_id, reason } => {
                write!(
                    f,
                    "report `{report_id}` has invalid delivery policy: {reason}"
                )
            }
            Self::InvalidBulkOperation {
                operation_id,
                reason,
            } => {
                write!(f, "bulk operation `{operation_id}` is invalid: {reason}")
            }
            Self::InvalidItemCount { operation, count } => {
                write!(f, "{operation} cannot target `{count}` items")
            }
            Self::JobsPlan { error } => write!(f, "{error}"),
        }
    }
}

impl Error for OpsModelError {}

impl From<JobsModelError> for OpsModelError {
    fn from(error: JobsModelError) -> Self {
        Self::JobsPlan { error }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDocumentKind {
    Page,
    Product,
    Collection,
    Event,
    EventSlot,
    Booking,
    Media,
    MembershipSubscription,
    Custom,
}

impl fmt::Display for SearchDocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page => f.write_str("page"),
            Self::Product => f.write_str("product"),
            Self::Collection => f.write_str("collection"),
            Self::Event => f.write_str("event"),
            Self::EventSlot => f.write_str("event_slot"),
            Self::Booking => f.write_str("booking"),
            Self::Media => f.write_str("media"),
            Self::MembershipSubscription => f.write_str("membership_subscription"),
            Self::Custom => f.write_str("custom"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFieldRole {
    Title,
    Summary,
    Body,
    Keyword,
    Facet,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchVisibility {
    Public,
    Authenticated,
    Capability(Capability),
}

impl SearchVisibility {
    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        match self {
            Self::Public => true,
            Self::Authenticated => !capabilities.is_empty(),
            Self::Capability(capability) => capabilities.contains(capability),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchInvalidationTrigger {
    Published,
    Updated,
    Unpublished,
    Deleted,
    ManualRebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchRebuildStrategy {
    OnInvalidate,
    Scheduled { interval: Duration },
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFieldContribution {
    pub id: SearchFieldId,
    pub source_path: String,
    pub role: SearchFieldRole,
    pub stored: bool,
    pub searchable: bool,
}

impl SearchFieldContribution {
    pub fn new(
        id: SearchFieldId,
        source_path: impl Into<String>,
        role: SearchFieldRole,
        stored: bool,
        searchable: bool,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            id,
            source_path: require_non_empty("search_field_source_path", source_path.into())?,
            role,
            stored,
            searchable,
        })
    }

    fn from_manifest_contribution(
        contribution: &ManifestSearchFieldContribution,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            SearchFieldId::new(contribution.id.clone())?,
            contribution.source_path.clone(),
            map_search_field_role(contribution.role),
            contribution.stored,
            contribution.searchable,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInvalidationRule {
    pub trigger: SearchInvalidationTrigger,
    pub reason: String,
}

impl SearchInvalidationRule {
    pub fn new(
        trigger: SearchInvalidationTrigger,
        reason: impl Into<String>,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            trigger,
            reason: require_non_empty("search_invalidation_reason", reason.into())?,
        })
    }

    fn from_manifest_rule(rule: &ManifestSearchInvalidationRule) -> Result<Self, OpsModelError> {
        Self::new(
            map_search_invalidation_trigger(rule.trigger),
            rule.reason.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexContribution {
    pub id: SearchIndexId,
    pub source_module: String,
    pub document_kind: SearchDocumentKind,
    pub visibility: SearchVisibility,
    pub publication_required: bool,
    pub fields: Vec<SearchFieldContribution>,
    pub invalidation_rules: Vec<SearchInvalidationRule>,
    pub rebuild_strategy: SearchRebuildStrategy,
}

impl SearchIndexContribution {
    pub fn new(
        id: SearchIndexId,
        source_module: impl Into<String>,
        document_kind: SearchDocumentKind,
        visibility: SearchVisibility,
        publication_required: bool,
        fields: Vec<SearchFieldContribution>,
        invalidation_rules: Vec<SearchInvalidationRule>,
        rebuild_strategy: SearchRebuildStrategy,
    ) -> Result<Self, OpsModelError> {
        let source_module = require_non_empty("search_source_module", source_module.into())?;

        if fields.is_empty() {
            return Err(OpsModelError::InvalidSearchVisibility {
                index_id: id.to_string(),
                reason: "at least one indexed field is required".to_string(),
            });
        }

        if matches!(visibility, SearchVisibility::Public) && !publication_required {
            return Err(OpsModelError::InvalidSearchVisibility {
                index_id: id.to_string(),
                reason: "public search indexes must require publication state".to_string(),
            });
        }

        let mut seen_fields = HashSet::new();
        for field in &fields {
            if !seen_fields.insert(field.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateField {
                    index_id: id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            source_module,
            document_kind,
            visibility,
            publication_required,
            fields,
            invalidation_rules,
            rebuild_strategy,
        })
    }

    pub fn visible_to(&self, capabilities: &[Capability]) -> bool {
        self.visibility.allows(capabilities)
    }

    fn from_manifest_contribution(
        source_module: &str,
        contribution: &ManifestSearchIndexContribution,
    ) -> Result<Self, OpsModelError> {
        let fields = contribution
            .fields
            .iter()
            .map(SearchFieldContribution::from_manifest_contribution)
            .collect::<Result<Vec<_>, _>>()?;
        let invalidation_rules = contribution
            .invalidation_rules
            .iter()
            .map(SearchInvalidationRule::from_manifest_rule)
            .collect::<Result<Vec<_>, _>>()?;

        Self::new(
            SearchIndexId::new(contribution.id.clone())?,
            source_module,
            map_search_document_kind(contribution.document_kind),
            map_search_visibility(contribution.visibility),
            contribution.publication_required,
            fields,
            invalidation_rules,
            map_search_rebuild_strategy(contribution.rebuild_strategy),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCatalog {
    pub contributions: Vec<SearchIndexContribution>,
}

impl SearchCatalog {
    pub fn new(contributions: Vec<SearchIndexContribution>) -> Self {
        Self { contributions }
    }

    pub fn standard() -> Self {
        Self {
            contributions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut contributions = Vec::new();
        for manifest in manifests {
            for contribution in &manifest.search_contributions {
                contributions.push(SearchIndexContribution::from_manifest_contribution(
                    &manifest.name,
                    contribution,
                )?);
            }
        }
        let catalog = Self::new(contributions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for contribution in &self.contributions {
            if !seen.insert(contribution.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "search index",
                    id: contribution.id.to_string(),
                });
            }
            SearchIndexContribution::new(
                contribution.id.clone(),
                contribution.source_module.clone(),
                contribution.document_kind,
                contribution.visibility,
                contribution.publication_required,
                contribution.fields.clone(),
                contribution.invalidation_rules.clone(),
                contribution.rebuild_strategy,
            )?;
        }

        Ok(())
    }

    pub fn contribution(&self, id: &SearchIndexId) -> Option<&SearchIndexContribution> {
        self.contributions
            .iter()
            .find(|contribution| &contribution.id == id)
    }

    pub fn visible_to(&self, capabilities: &[Capability]) -> Vec<&SearchIndexContribution> {
        self.contributions
            .iter()
            .filter(|contribution| contribution.visible_to(capabilities))
            .collect()
    }
}

impl Default for SearchCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Csv,
    Json,
    Pdf,
}

impl ReportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Pdf => "pdf",
        }
    }
}

impl fmt::Display for ReportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv => f.write_str("csv"),
            Self::Json => f.write_str("json"),
            Self::Pdf => f.write_str("pdf"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSensitivity {
    Public,
    Internal,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportDeliveryMode {
    PublicObjectStore,
    SignedUrl,
    InternalOnly,
}

impl ReportDeliveryMode {
    pub fn is_public(self) -> bool {
        matches!(self, Self::PublicObjectStore)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportParameter {
    pub key: String,
    pub value: String,
}

impl ReportParameter {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, OpsModelError> {
        Ok(Self {
            key: require_non_empty("report_parameter_key", key.into())?,
            value: require_non_empty("report_parameter_value", value.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDefinition {
    pub id: ReportId,
    pub source_module: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub format: ReportFormat,
    pub sensitivity: ReportSensitivity,
    pub delivery_mode: ReportDeliveryMode,
    pub export_prefix: String,
    pub retry_policy: RetryPolicy,
}

impl ReportDefinition {
    pub fn new(
        id: ReportId,
        source_module: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        format: ReportFormat,
        sensitivity: ReportSensitivity,
        delivery_mode: ReportDeliveryMode,
        export_prefix: impl Into<String>,
        retry_policy: RetryPolicy,
    ) -> Result<Self, OpsModelError> {
        let source_module = require_non_empty("report_source_module", source_module.into())?;
        let title = require_non_empty("report_title", title.into())?;
        let export_prefix = require_non_empty("report_export_prefix", export_prefix.into())?;

        if matches!(
            (sensitivity, delivery_mode),
            (ReportSensitivity::Public, ReportDeliveryMode::InternalOnly)
        ) {
            return Err(OpsModelError::InvalidReportDelivery {
                report_id: id.to_string(),
                reason:
                    "public reports should be delivered through a public object store or signed URL"
                        .to_string(),
            });
        }

        if matches!(sensitivity, ReportSensitivity::Restricted)
            && matches!(delivery_mode, ReportDeliveryMode::PublicObjectStore)
        {
            return Err(OpsModelError::InvalidReportDelivery {
                report_id: id.to_string(),
                reason: "restricted reports cannot be delivered through a public object store"
                    .to_string(),
            });
        }

        Ok(Self {
            id,
            source_module,
            title,
            description,
            required_capability,
            format,
            sensitivity,
            delivery_mode,
            export_prefix,
            retry_policy,
        })
    }

    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        capabilities.contains(&self.required_capability)
    }

    fn from_manifest_definition(
        source_module: &str,
        definition: &ManifestReportDefinition,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            ReportId::new(definition.id.clone())?,
            source_module,
            definition.title.clone(),
            definition.description.clone(),
            definition.required_capability,
            map_report_format(definition.format),
            map_report_sensitivity(definition.sensitivity),
            map_report_delivery_mode(definition.delivery_mode),
            definition.export_prefix.clone(),
            definition.retry_policy.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportExportRequest {
    pub export_id: ReportExportId,
    pub report_id: ReportId,
    pub requested_by: String,
    pub requested_at: JobInstant,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<IdempotencyKey>,
    pub operator_capabilities: Vec<Capability>,
    pub parameters: Vec<ReportParameter>,
}

impl ReportExportRequest {
    pub fn new(
        export_id: ReportExportId,
        report_id: ReportId,
        requested_by: impl Into<String>,
        requested_at: JobInstant,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            export_id,
            report_id,
            requested_by: require_non_empty("report_requested_by", requested_by.into())?,
            requested_at,
            scheduled_for: None,
            idempotency_key: None,
            operator_capabilities: Vec::new(),
            parameters: Vec::new(),
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.operator_capabilities.push(capability);
        self
    }

    pub fn with_parameter(mut self, parameter: ReportParameter) -> Self {
        self.parameters.push(parameter);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportExportPlan {
    pub definition: ReportDefinition,
    pub job: JobSpec,
    pub planned_job: PlannedJob,
    pub output_object_key: String,
    pub parameters: Vec<ReportParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportCatalog {
    pub definitions: Vec<ReportDefinition>,
}

impl ReportCatalog {
    pub fn new(definitions: Vec<ReportDefinition>) -> Self {
        Self { definitions }
    }

    pub fn standard() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut definitions = Vec::new();
        for manifest in manifests {
            for definition in &manifest.report_definitions {
                definitions.push(ReportDefinition::from_manifest_definition(
                    &manifest.name,
                    definition,
                )?);
            }
        }
        let catalog = Self::new(definitions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for definition in &self.definitions {
            if !seen.insert(definition.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "report",
                    id: definition.id.to_string(),
                });
            }

            ReportDefinition::new(
                definition.id.clone(),
                definition.source_module.clone(),
                definition.title.clone(),
                definition.description.clone(),
                definition.required_capability,
                definition.format,
                definition.sensitivity,
                definition.delivery_mode,
                definition.export_prefix.clone(),
                definition.retry_policy.clone(),
            )?;
        }

        Ok(())
    }

    pub fn definition(&self, id: &ReportId) -> Option<&ReportDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.id == id)
    }
}

impl Default for ReportCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationKind {
    Publish,
    Unpublish,
    Reindex,
    Export,
    Cancel,
    CheckIn,
    Custom,
}

impl fmt::Display for BulkOperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Publish => f.write_str("publish"),
            Self::Unpublish => f.write_str("unpublish"),
            Self::Reindex => f.write_str("reindex"),
            Self::Export => f.write_str("export"),
            Self::Cancel => f.write_str("cancel"),
            Self::CheckIn => f.write_str("check_in"),
            Self::Custom => f.write_str("custom"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationScope {
    Cms,
    Commerce,
    Memberships,
    Events,
    Media,
    Search,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationDefinition {
    pub id: BulkOperationId,
    pub source_module: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub kind: BulkOperationKind,
    pub scope: BulkOperationScope,
    pub retry_policy: RetryPolicy,
    pub max_items: Option<usize>,
    pub requires_idempotency_key: bool,
}

impl BulkOperationDefinition {
    pub fn new(
        id: BulkOperationId,
        source_module: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        kind: BulkOperationKind,
        scope: BulkOperationScope,
        retry_policy: RetryPolicy,
        max_items: Option<usize>,
        requires_idempotency_key: bool,
    ) -> Result<Self, OpsModelError> {
        let source_module = require_non_empty("bulk_source_module", source_module.into())?;
        let title = require_non_empty("bulk_title", title.into())?;

        if let Some(max_items) = max_items {
            if max_items == 0 {
                return Err(OpsModelError::InvalidBulkOperation {
                    operation_id: id.to_string(),
                    reason: "max_items must be greater than zero".to_string(),
                });
            }
        }

        if retry_policy.is_retrying() && !requires_idempotency_key {
            return Err(OpsModelError::InvalidBulkOperation {
                operation_id: id.to_string(),
                reason: "retrying bulk operations must require an idempotency key".to_string(),
            });
        }

        Ok(Self {
            id,
            source_module,
            title,
            description,
            required_capability,
            kind,
            scope,
            retry_policy,
            max_items,
            requires_idempotency_key,
        })
    }

    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        capabilities.contains(&self.required_capability)
    }

    fn from_manifest_definition(
        source_module: &str,
        definition: &ManifestBulkOperationDefinition,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            BulkOperationId::new(definition.id.clone())?,
            source_module,
            definition.title.clone(),
            definition.description.clone(),
            definition.required_capability,
            map_bulk_operation_kind(definition.kind),
            map_bulk_operation_scope(definition.scope),
            definition.retry_policy.clone(),
            definition.max_items,
            definition.requires_idempotency_key,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationRequest {
    pub execution_id: BulkExecutionId,
    pub definition_id: BulkOperationId,
    pub requested_by: String,
    pub requested_at: JobInstant,
    pub target_count: usize,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<IdempotencyKey>,
    pub operator_capabilities: Vec<Capability>,
    pub dry_run: bool,
}

impl BulkOperationRequest {
    pub fn new(
        execution_id: BulkExecutionId,
        definition_id: BulkOperationId,
        requested_by: impl Into<String>,
        requested_at: JobInstant,
        target_count: usize,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            execution_id,
            definition_id,
            requested_by: require_non_empty("bulk_requested_by", requested_by.into())?,
            requested_at,
            target_count,
            scheduled_for: None,
            idempotency_key: None,
            operator_capabilities: Vec::new(),
            dry_run: false,
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.operator_capabilities.push(capability);
        self
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationPlan {
    pub definition: BulkOperationDefinition,
    pub job: JobSpec,
    pub planned_job: PlannedJob,
    pub dry_run: bool,
    pub target_count: usize,
    pub audit_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkCatalog {
    pub definitions: Vec<BulkOperationDefinition>,
}

impl BulkCatalog {
    pub fn new(definitions: Vec<BulkOperationDefinition>) -> Self {
        Self { definitions }
    }

    pub fn standard() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut definitions = Vec::new();
        for manifest in manifests {
            for definition in &manifest.bulk_operations {
                definitions.push(BulkOperationDefinition::from_manifest_definition(
                    &manifest.name,
                    definition,
                )?);
            }
        }
        let catalog = Self::new(definitions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for definition in &self.definitions {
            if !seen.insert(definition.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "bulk operation",
                    id: definition.id.to_string(),
                });
            }

            BulkOperationDefinition::new(
                definition.id.clone(),
                definition.source_module.clone(),
                definition.title.clone(),
                definition.description.clone(),
                definition.required_capability,
                definition.kind,
                definition.scope,
                definition.retry_policy.clone(),
                definition.max_items,
                definition.requires_idempotency_key,
            )?;
        }

        Ok(())
    }

    pub fn definition(&self, id: &BulkOperationId) -> Option<&BulkOperationDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.id == id)
    }
}

impl Default for BulkCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsCatalog {
    pub search: SearchCatalog,
    pub reports: ReportCatalog,
    pub bulk: BulkCatalog,
}

impl OpsCatalog {
    pub fn new(search: SearchCatalog, reports: ReportCatalog, bulk: BulkCatalog) -> Self {
        Self {
            search,
            reports,
            bulk,
        }
    }

    pub fn standard() -> Self {
        Self {
            search: SearchCatalog::standard(),
            reports: ReportCatalog::standard(),
            bulk: BulkCatalog::standard(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let catalog = Self {
            search: SearchCatalog::from_manifests(manifests)?,
            reports: ReportCatalog::from_manifests(manifests)?,
            bulk: BulkCatalog::from_manifests(manifests)?,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        self.search.validate()?;
        self.reports.validate()?;
        self.bulk.validate()?;
        Ok(())
    }
}

impl Default for OpsCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsPlanner {
    jobs: JobsPlanner,
    catalog: OpsCatalog,
}

impl OpsPlanner {
    pub fn new(runtime: JobsRuntime, catalog: OpsCatalog) -> Result<Self, OpsModelError> {
        catalog.validate()?;
        Ok(Self {
            jobs: runtime.planner(),
            catalog,
        })
    }

    pub fn jobs_planner(&self) -> &JobsPlanner {
        &self.jobs
    }

    pub fn catalog(&self) -> &OpsCatalog {
        &self.catalog
    }

    pub fn plan_report_export(
        &self,
        request: ReportExportRequest,
    ) -> Result<ReportExportPlan, OpsModelError> {
        let definition = self
            .catalog
            .reports
            .definition(&request.report_id)
            .ok_or_else(|| OpsModelError::DuplicateIdentifier {
                kind: "report",
                id: request.report_id.to_string(),
            })?;

        if !definition.allows(&request.operator_capabilities) {
            return Err(OpsModelError::MissingCapability {
                operation: "report export",
                required: definition.required_capability,
            });
        }

        let queue_topology = self.jobs.describe_queue_topology();
        let queue = if request.scheduled_for.is_some() {
            queue_topology.scheduled_queue.clone()
        } else {
            queue_topology.work_queue.clone()
        };

        let job_id = JobId::new(request.export_id.as_str().to_string())?;
        let job_name = JobName::new(format!("report.export.{}", definition.id.as_str()))?;
        let mut retry_policy = definition.retry_policy.clone();
        retry_policy =
            retry_policy.with_dead_letter_queue(queue_topology.dead_letter_queue.clone());

        let mut spec = JobSpec::new(
            job_id,
            job_name,
            queue,
            format!(
                "report export for `{}` in format `{}`",
                definition.id, definition.format
            ),
        )?;
        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }
        spec = spec.with_retry_policy(retry_policy);
        if let Some(key) = request.idempotency_key.clone() {
            spec = spec.with_idempotency_key(key);
        }

        let planned_job = self.jobs.plan_job(spec.clone(), request.requested_at)?;

        Ok(ReportExportPlan {
            definition: definition.clone(),
            job: spec,
            planned_job,
            output_object_key: format!(
                "{}/{}.{}",
                definition.export_prefix,
                request.export_id,
                definition.format.extension()
            ),
            parameters: request.parameters,
        })
    }

    pub fn plan_bulk_operation(
        &self,
        request: BulkOperationRequest,
    ) -> Result<BulkOperationPlan, OpsModelError> {
        let definition = self
            .catalog
            .bulk
            .definition(&request.definition_id)
            .ok_or_else(|| OpsModelError::DuplicateIdentifier {
                kind: "bulk operation",
                id: request.definition_id.to_string(),
            })?;

        if !definition.allows(&request.operator_capabilities) {
            return Err(OpsModelError::MissingCapability {
                operation: "bulk operation",
                required: definition.required_capability,
            });
        }

        if request.target_count == 0 {
            return Err(OpsModelError::InvalidItemCount {
                operation: "bulk operation",
                count: request.target_count,
            });
        }

        if let Some(max_items) = definition.max_items {
            if request.target_count > max_items {
                return Err(OpsModelError::InvalidItemCount {
                    operation: "bulk operation",
                    count: request.target_count,
                });
            }
        }

        if definition.requires_idempotency_key && request.idempotency_key.is_none() {
            return Err(OpsModelError::InvalidBulkOperation {
                operation_id: definition.id.to_string(),
                reason: "idempotency key is required for retry-safe execution".to_string(),
            });
        }

        let queue_topology = self.jobs.describe_queue_topology();
        let queue = if request.scheduled_for.is_some() {
            queue_topology.scheduled_queue.clone()
        } else {
            queue_topology.work_queue.clone()
        };

        let job_id = JobId::new(request.execution_id.as_str().to_string())?;
        let job_name = JobName::new(format!("bulk.{}", definition.id.as_str()))?;
        let mut retry_policy = definition.retry_policy.clone();
        retry_policy =
            retry_policy.with_dead_letter_queue(queue_topology.dead_letter_queue.clone());

        let mut spec = JobSpec::new(
            job_id,
            job_name,
            queue,
            format!(
                "bulk `{}` on `{}` items",
                definition.kind, request.target_count
            ),
        )?;
        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }
        spec = spec.with_retry_policy(retry_policy);
        if let Some(key) = request.idempotency_key.clone() {
            spec = spec.with_idempotency_key(key);
        }

        let planned_job = self.jobs.plan_job(spec.clone(), request.requested_at)?;

        Ok(BulkOperationPlan {
            definition: definition.clone(),
            job: spec,
            planned_job,
            dry_run: request.dry_run,
            target_count: request.target_count,
            audit_message: format!(
                "bulk `{}` requested by `{}` for `{}` items",
                definition.id, request.requested_by, request.target_count
            ),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModule {
    name: String,
    config_namespace: String,
}

impl OpsModule {
    pub fn new() -> Self {
        Self {
            name: "ops".to_string(),
            config_namespace: "ops".to_string(),
        }
    }

    pub fn planner(
        &self,
        runtime: JobsRuntime,
        manifests: &[ModuleManifest],
    ) -> Result<OpsPlanner, OpsModelError> {
        OpsPlanner::new(runtime, OpsCatalog::from_manifests(manifests)?)
    }
}

impl Default for OpsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for OpsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::AdminAuditRead,
                Capability::SystemModuleManage,
            ])
            .with_optional_capabilities(vec![
                Capability::CmsPageRead,
                Capability::CmsPageEdit,
                Capability::CmsPagePublish,
                Capability::CmsNavigationEdit,
                Capability::CatalogProductRead,
                Capability::CatalogProductEdit,
                Capability::CatalogCollectionEdit,
                Capability::OrderRead,
                Capability::OrderRefundIssue,
                Capability::MembershipSubscriptionManage,
                Capability::MembershipTierEdit,
                Capability::EventsEventPublish,
                Capability::EventsSlotManage,
                Capability::EventsBookingCreate,
                Capability::EventsBookingCheckIn,
                Capability::AssetRead,
                Capability::AssetReadPublic,
                Capability::AssetPublish,
                Capability::AssetReplace,
                Capability::AssetManageStorage,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::required(
                    Capability::AdminAuditRead,
                    ["audit_entry"],
                ),
                CapabilityContract::required(
                    Capability::SystemModuleManage,
                    ["admin_module"],
                ),
                CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
                CapabilityContract::optional(Capability::CmsPageEdit, ["page"]),
                CapabilityContract::optional(Capability::CmsPagePublish, ["page"]),
                CapabilityContract::optional(
                    Capability::CmsNavigationEdit,
                    ["navigation"],
                ),
                CapabilityContract::optional(
                    Capability::CatalogProductRead,
                    ["product"],
                ),
                CapabilityContract::optional(
                    Capability::CatalogProductEdit,
                    ["product"],
                ),
                CapabilityContract::optional(
                    Capability::CatalogCollectionEdit,
                    ["collection"],
                ),
                CapabilityContract::optional(Capability::OrderRead, ["order"]),
                CapabilityContract::optional(Capability::OrderRefundIssue, ["order"]),
                CapabilityContract::optional(
                    Capability::MembershipSubscriptionManage,
                    ["subscription"],
                ),
                CapabilityContract::optional(
                    Capability::MembershipTierEdit,
                    ["membership_tier"],
                ),
                CapabilityContract::optional(
                    Capability::EventsEventPublish,
                    ["event"],
                ),
                CapabilityContract::optional(
                    Capability::EventsSlotManage,
                    ["event_slot"],
                ),
                CapabilityContract::optional(
                    Capability::EventsBookingCreate,
                    ["booking"],
                ),
                CapabilityContract::optional(
                    Capability::EventsBookingCheckIn,
                    ["booking"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
                CapabilityContract::optional(
                    Capability::AssetReadPublic,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AssetPublish,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AssetReplace,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AssetManageStorage,
                    ["asset", "asset_folder", "media_library"],
                ),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::required(
                    "admin",
                    "Operational search, reporting, and bulk actions surface through the shared admin shell",
                ),
                ModuleDependency::optional(
                    "cms",
                    "Search and bulk publishing can index and mutate CMS content when that module is installed",
                ),
                ModuleDependency::optional(
                    "commerce",
                    "Reporting and bulk operations can project order and catalog activity when commerce is installed",
                ),
                ModuleDependency::optional(
                    "memberships",
                    "Operational reports can include subscription and entitlement state when memberships is installed",
                ),
                ModuleDependency::optional(
                    "events",
                    "Search and bulk workflows can operate on bookings and check-in state when events is installed",
                ),
                ModuleDependency::optional(
                    "media",
                    "Search and reporting can include managed assets and storage-policy inventory when media is installed",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Storage,
                CoreServiceDependency::Cache,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "ops.search",
                    10,
                    "Creates search projection and rebuild-cursor tables for first-party indexing",
                ),
                MigrationContract::new(
                    "ops.reports",
                    20,
                    "Creates report definition, export job, and output artifact metadata tables",
                ),
                MigrationContract::new(
                    "ops.bulk",
                    30,
                    "Creates bulk-operation intent, idempotency, and audit coordination tables",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new(
                    "ops.search",
                    RouteSurfaceKind::AdminPage,
                    "/admin/search",
                )
                .gated_by(Capability::AdminShellAccess),
                RouteSurface::new(
                    "ops.reports",
                    RouteSurfaceKind::AdminPage,
                    "/admin/reports",
                )
                .gated_by(Capability::AdminAuditRead),
                RouteSurface::new(
                    "ops.bulk",
                    RouteSurfaceKind::AdminAction,
                    "/admin/bulk",
                )
                .gated_by(Capability::SystemModuleManage),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "ops.search.rebuild",
                    JobTriggerKind::DomainEvent,
                    true,
                    "Rebuilds or repairs first-party search projections as domain records change",
                ),
                JobContract::new(
                    "ops.report.export",
                    JobTriggerKind::Operator,
                    true,
                    "Runs asynchronous report exports and persists their output artifacts safely",
                ),
                JobContract::new(
                    "ops.bulk.execute",
                    JobTriggerKind::Operator,
                    true,
                    "Executes audited bulk workflows behind idempotent job envelopes",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "cms.page.published",
                    Some("ops.search.rebuild"),
                    "Refreshes search projections after editorial publication changes",
                ),
                EventSubscription::new(
                    "commerce.order.paid",
                    Some("ops.report.export"),
                    "Enables scheduled or on-demand reporting to capture completed transactional activity",
                ),
                EventSubscription::new(
                    "events.booking.checked-in",
                    Some("ops.bulk.execute"),
                    "Keeps operational bulk and reporting views consistent with live attendance changes",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::SearchIndex,
                    "ops.search",
                    "Collects explicit indexing contributions from official modules and customer app extensions",
                ),
                IntegrationPoint::new(
                    IntegrationKind::AdminWorkflow,
                    "ops.bulk",
                    "Adds report export and bulk workflow surfaces into the shared admin shell",
                ),
                IntegrationPoint::new(
                    IntegrationKind::StoragePolicy,
                    "ops.report-output",
                    "Routes generated report artifacts through the shared storage-policy and delivery model",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AuditedBulkActions,
            ])
            .with_extension_slots(vec![
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::AdminWidget,
                    "ops.report.dashboard",
                    "Allows bounded customer widgets to contribute operator metrics and report affordances",
                ),
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::Job,
                    "ops.search.adapter",
                    "Allows search backends to participate through explicit background job contracts",
                ),
            ])
            .with_admin_resources(vec![
                AdminResourceContribution::new(
                    "ops.search",
                    "/admin/search",
                    "Search",
                    "Search",
                    AdminNavigationSection::System,
                    AdminContributionKind::ResourceIndex,
                    Capability::AdminShellAccess,
                ),
                AdminResourceContribution::new(
                    "ops.reports",
                    "/admin/reports",
                    "Reports",
                    "Reports",
                    AdminNavigationSection::System,
                    AdminContributionKind::ResourceIndex,
                    Capability::AdminAuditRead,
                ),
                AdminResourceContribution::new(
                    "ops.bulk",
                    "/admin/bulk",
                    "Bulk operations",
                    "Bulk",
                    AdminNavigationSection::System,
                    AdminContributionKind::Workflow,
                    Capability::SystemModuleManage,
                ),
            ])
            .with_report_definitions(vec![ManifestReportDefinition::new(
                "report.ops.search-health",
                "Search health",
                Some("Operational visibility into index freshness, drift, and rebuild lag".to_string()),
                Capability::AdminAuditRead,
                ManifestReportFormat::Json,
                ManifestReportSensitivity::Internal,
                ManifestReportDeliveryMode::SignedUrl,
                "reports/ops/search",
                default_retry_policy(),
            )])
            .with_bulk_operations(vec![
                ManifestBulkOperationDefinition::new(
                    "bulk.search.reindex",
                    "Reindex search",
                    Some("Queues a coordinated rebuild across declared search indexes".to_string()),
                    Capability::SystemModuleManage,
                    ManifestBulkOperationKind::Reindex,
                    ManifestBulkOperationScope::Search,
                    default_retry_policy(),
                    Some(100),
                    true,
                ),
                ManifestBulkOperationDefinition::new(
                    "bulk.reports.export",
                    "Bulk export reports",
                    Some("Queues exports for multiple reports without request-time blocking".to_string()),
                    Capability::AdminAuditRead,
                    ManifestBulkOperationKind::Export,
                    ManifestBulkOperationScope::System,
                    default_retry_policy(),
                    Some(50),
                    true,
                ),
            ])
            .with_http_surfaces(vec![
                HttpSurfaceContribution::page(
                    "ops.search",
                    HttpSurfaceArea::Admin,
                    "/admin/search",
                    "ops/search",
                )
                .gated_by(Capability::AdminShellAccess),
                HttpSurfaceContribution::page(
                    "ops.reports",
                    HttpSurfaceArea::Admin,
                    "/admin/reports",
                    "ops/reports",
                )
                .gated_by(Capability::AdminAuditRead),
                HttpSurfaceContribution::json(
                    "ops.bulk",
                    HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/bulk",
                    202,
                    std::collections::BTreeMap::from([(
                        "status".to_string(),
                        "queued".to_string(),
                    )]),
                )
                .gated_by(Capability::SystemModuleManage),
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.ops.search",
            "Declarative search indexing contributions, visibility rules, and rebuild metadata",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.reports",
            "Asynchronous report definitions, export planning, and delivery policies",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.bulk",
            "Capability-gated bulk operations with audit-ready, idempotent job planning",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.jobs",
            "Jobs-backed execution planning for reports and bulk workflows",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.audit",
            "Operator visibility into search, reporting, and bulk action plans",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("ops_search").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create search projection and rebuild cursor storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS ops_search_projection (id TEXT PRIMARY KEY, document_type TEXT NOT NULL, visibility TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("ops migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("ops_reports").expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create report definition and export artifact storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS ops_reports (id TEXT PRIMARY KEY, format TEXT NOT NULL, output_path TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("ops migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("ops_bulk").expect("constant migration id is valid"),
                owner,
                30,
                "Create bulk workflow intent and idempotency storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS ops_bulk_operations (id TEXT PRIMARY KEY, action TEXT NOT NULL, idempotency_key TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("ops migration ids are unique");
        Some(plan)
    }
}

fn map_search_document_kind(kind: ManifestSearchDocumentKind) -> SearchDocumentKind {
    match kind {
        ManifestSearchDocumentKind::Page => SearchDocumentKind::Page,
        ManifestSearchDocumentKind::Product => SearchDocumentKind::Product,
        ManifestSearchDocumentKind::Collection => SearchDocumentKind::Collection,
        ManifestSearchDocumentKind::Event => SearchDocumentKind::Event,
        ManifestSearchDocumentKind::EventSlot => SearchDocumentKind::EventSlot,
        ManifestSearchDocumentKind::Booking => SearchDocumentKind::Booking,
        ManifestSearchDocumentKind::Media => SearchDocumentKind::Media,
        ManifestSearchDocumentKind::MembershipSubscription => {
            SearchDocumentKind::MembershipSubscription
        }
        ManifestSearchDocumentKind::Custom => SearchDocumentKind::Custom,
    }
}

fn map_search_field_role(role: ManifestSearchFieldRole) -> SearchFieldRole {
    match role {
        ManifestSearchFieldRole::Title => SearchFieldRole::Title,
        ManifestSearchFieldRole::Summary => SearchFieldRole::Summary,
        ManifestSearchFieldRole::Body => SearchFieldRole::Body,
        ManifestSearchFieldRole::Keyword => SearchFieldRole::Keyword,
        ManifestSearchFieldRole::Facet => SearchFieldRole::Facet,
        ManifestSearchFieldRole::Metadata => SearchFieldRole::Metadata,
    }
}

fn map_search_visibility(visibility: ManifestSearchVisibility) -> SearchVisibility {
    match visibility {
        ManifestSearchVisibility::Public => SearchVisibility::Public,
        ManifestSearchVisibility::Authenticated => SearchVisibility::Authenticated,
        ManifestSearchVisibility::Capability(capability) => {
            SearchVisibility::Capability(capability)
        }
    }
}

fn map_search_invalidation_trigger(
    trigger: ManifestSearchInvalidationTrigger,
) -> SearchInvalidationTrigger {
    match trigger {
        ManifestSearchInvalidationTrigger::Published => SearchInvalidationTrigger::Published,
        ManifestSearchInvalidationTrigger::Updated => SearchInvalidationTrigger::Updated,
        ManifestSearchInvalidationTrigger::Unpublished => SearchInvalidationTrigger::Unpublished,
        ManifestSearchInvalidationTrigger::Deleted => SearchInvalidationTrigger::Deleted,
        ManifestSearchInvalidationTrigger::ManualRebuild => {
            SearchInvalidationTrigger::ManualRebuild
        }
    }
}

fn map_search_rebuild_strategy(strategy: ManifestSearchRebuildStrategy) -> SearchRebuildStrategy {
    match strategy {
        ManifestSearchRebuildStrategy::OnInvalidate => SearchRebuildStrategy::OnInvalidate,
        ManifestSearchRebuildStrategy::Scheduled { interval } => {
            SearchRebuildStrategy::Scheduled { interval }
        }
        ManifestSearchRebuildStrategy::ManualOnly => SearchRebuildStrategy::ManualOnly,
    }
}

fn map_report_format(format: ManifestReportFormat) -> ReportFormat {
    match format {
        ManifestReportFormat::Csv => ReportFormat::Csv,
        ManifestReportFormat::Json => ReportFormat::Json,
        ManifestReportFormat::Pdf => ReportFormat::Pdf,
    }
}

fn map_report_sensitivity(sensitivity: ManifestReportSensitivity) -> ReportSensitivity {
    match sensitivity {
        ManifestReportSensitivity::Public => ReportSensitivity::Public,
        ManifestReportSensitivity::Internal => ReportSensitivity::Internal,
        ManifestReportSensitivity::Restricted => ReportSensitivity::Restricted,
    }
}

fn map_report_delivery_mode(mode: ManifestReportDeliveryMode) -> ReportDeliveryMode {
    match mode {
        ManifestReportDeliveryMode::PublicObjectStore => ReportDeliveryMode::PublicObjectStore,
        ManifestReportDeliveryMode::SignedUrl => ReportDeliveryMode::SignedUrl,
        ManifestReportDeliveryMode::InternalOnly => ReportDeliveryMode::InternalOnly,
    }
}

fn map_bulk_operation_kind(kind: ManifestBulkOperationKind) -> BulkOperationKind {
    match kind {
        ManifestBulkOperationKind::Publish => BulkOperationKind::Publish,
        ManifestBulkOperationKind::Unpublish => BulkOperationKind::Unpublish,
        ManifestBulkOperationKind::Reindex => BulkOperationKind::Reindex,
        ManifestBulkOperationKind::Export => BulkOperationKind::Export,
        ManifestBulkOperationKind::Cancel => BulkOperationKind::Cancel,
        ManifestBulkOperationKind::CheckIn => BulkOperationKind::CheckIn,
        ManifestBulkOperationKind::Custom => BulkOperationKind::Custom,
    }
}

fn map_bulk_operation_scope(scope: ManifestBulkOperationScope) -> BulkOperationScope {
    match scope {
        ManifestBulkOperationScope::Cms => BulkOperationScope::Cms,
        ManifestBulkOperationScope::Commerce => BulkOperationScope::Commerce,
        ManifestBulkOperationScope::Memberships => BulkOperationScope::Memberships,
        ManifestBulkOperationScope::Events => BulkOperationScope::Events,
        ManifestBulkOperationScope::Media => BulkOperationScope::Media,
        ManifestBulkOperationScope::Search => BulkOperationScope::Search,
        ManifestBulkOperationScope::System => BulkOperationScope::System,
    }
}

fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}

fn validate_token(field: &'static str, value: String) -> Result<String, OpsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(OpsModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(OpsModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, OpsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(OpsModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jobs_runtime() -> JobsRuntime {
        JobsRuntime::from_config(&davenda_config::JobsConfig {
            backend: davenda_config::JobBackend::Redis,
            retry_limit: 3,
        })
        .expect("valid jobs runtime")
    }

    fn sample_manifests() -> Vec<ModuleManifest> {
        vec![
            ModuleManifest::new("cms")
                .with_search_contributions(vec![ManifestSearchIndexContribution::new(
                    "search.cms.pages",
                    ManifestSearchDocumentKind::Page,
                    ManifestSearchVisibility::Public,
                    true,
                    vec![ManifestSearchFieldContribution::new(
                        "title",
                        "title",
                        ManifestSearchFieldRole::Title,
                        true,
                        true,
                    )],
                    vec![ManifestSearchInvalidationRule::new(
                        ManifestSearchInvalidationTrigger::Published,
                        "page published",
                    )],
                    ManifestSearchRebuildStrategy::OnInvalidate,
                )])
                .with_bulk_operations(vec![ManifestBulkOperationDefinition::new(
                    "bulk.cms.publish",
                    "Bulk publish pages",
                    Some("Publishes editorial pages".to_string()),
                    Capability::CmsPagePublish,
                    ManifestBulkOperationKind::Publish,
                    ManifestBulkOperationScope::Cms,
                    default_retry_policy(),
                    Some(100),
                    true,
                )]),
            ModuleManifest::new("commerce")
                .with_search_contributions(vec![ManifestSearchIndexContribution::new(
                    "search.catalog.products",
                    ManifestSearchDocumentKind::Product,
                    ManifestSearchVisibility::Public,
                    true,
                    vec![ManifestSearchFieldContribution::new(
                        "name",
                        "name",
                        ManifestSearchFieldRole::Title,
                        true,
                        true,
                    )],
                    vec![ManifestSearchInvalidationRule::new(
                        ManifestSearchInvalidationTrigger::Published,
                        "product published",
                    )],
                    ManifestSearchRebuildStrategy::OnInvalidate,
                )])
                .with_report_definitions(vec![ManifestReportDefinition::new(
                    "report.orders.summary",
                    "Orders summary",
                    Some("Operational summary of captured orders".to_string()),
                    Capability::OrderRead,
                    ManifestReportFormat::Csv,
                    ManifestReportSensitivity::Restricted,
                    ManifestReportDeliveryMode::InternalOnly,
                    "reports/orders",
                    default_retry_policy(),
                )]),
            ModuleManifest::new("events")
                .with_search_contributions(vec![ManifestSearchIndexContribution::new(
                    "search.events.bookings",
                    ManifestSearchDocumentKind::Booking,
                    ManifestSearchVisibility::Capability(Capability::EventsBookingCheckIn),
                    false,
                    vec![ManifestSearchFieldContribution::new(
                        "attendee",
                        "attendee.display_name",
                        ManifestSearchFieldRole::Title,
                        true,
                        true,
                    )],
                    vec![ManifestSearchInvalidationRule::new(
                        ManifestSearchInvalidationTrigger::Updated,
                        "booking changed",
                    )],
                    ManifestSearchRebuildStrategy::ManualOnly,
                )])
                .with_bulk_operations(vec![ManifestBulkOperationDefinition::new(
                    "bulk.events.check-in",
                    "Bulk check in bookings",
                    Some("Checks in event bookings".to_string()),
                    Capability::EventsBookingCheckIn,
                    ManifestBulkOperationKind::CheckIn,
                    ManifestBulkOperationScope::Events,
                    default_retry_policy(),
                    Some(500),
                    true,
                )]),
            ModuleManifest::new("media").with_search_contributions(vec![
                ManifestSearchIndexContribution::new(
                    "search.media",
                    ManifestSearchDocumentKind::Media,
                    ManifestSearchVisibility::Public,
                    true,
                    vec![ManifestSearchFieldContribution::new(
                        "title",
                        "title",
                        ManifestSearchFieldRole::Title,
                        true,
                        true,
                    )],
                    vec![ManifestSearchInvalidationRule::new(
                        ManifestSearchInvalidationTrigger::Published,
                        "asset published",
                    )],
                    ManifestSearchRebuildStrategy::OnInvalidate,
                ),
            ]),
            OpsModule::new().manifest(),
        ]
    }

    #[test]
    fn catalog_exposes_search_visibility_rules() {
        let manifests = sample_manifests();
        let catalog = OpsCatalog::from_manifests(&manifests).expect("catalog");
        catalog
            .validate()
            .expect("composed catalog should validate");

        let visible = catalog.search.visible_to(&[]);
        assert!(
            visible
                .iter()
                .any(|index| index.id.as_str() == "search.cms.pages")
        );
        assert!(
            visible
                .iter()
                .any(|index| index.id.as_str() == "search.media")
        );
        assert!(
            !visible
                .iter()
                .any(|index| index.id.as_str() == "search.events.bookings")
        );
    }

    #[test]
    fn report_exports_are_planned_as_async_jobs_with_output_metadata() {
        let planner = OpsPlanner::new(
            jobs_runtime(),
            OpsCatalog::from_manifests(&sample_manifests()).expect("catalog"),
        )
        .expect("planner");
        let request = ReportExportRequest::new(
            ReportExportId::new("export-1").expect("valid id"),
            ReportId::new("report.orders.summary").expect("valid id"),
            "operator-1",
            JobInstant::from_unix_seconds(100),
        )
        .expect("request")
        .with_capability(Capability::OrderRead)
        .with_idempotency_key(IdempotencyKey::new("orders:summary:v1").expect("valid key"));

        let plan = planner.plan_report_export(request).expect("export plan");
        assert_eq!(plan.job.queue.as_str(), "jobs.work");
        assert_eq!(plan.definition.id.as_str(), "report.orders.summary");
        assert!(plan.planned_job.idempotency_key.is_some());
        assert_eq!(plan.output_object_key, "reports/orders/export-1.csv");
    }

    #[test]
    fn bulk_operations_require_their_capability_and_idempotency() {
        let planner = OpsPlanner::new(
            jobs_runtime(),
            OpsCatalog::from_manifests(&sample_manifests()).expect("catalog"),
        )
        .expect("planner");
        let request = BulkOperationRequest::new(
            BulkExecutionId::new("bulk-1").expect("valid id"),
            BulkOperationId::new("bulk.events.check-in").expect("valid id"),
            "operator-2",
            JobInstant::from_unix_seconds(200),
            12,
        )
        .expect("request");

        let missing = planner.plan_bulk_operation(request.clone()).unwrap_err();
        assert!(matches!(
            missing,
            OpsModelError::MissingCapability {
                operation: "bulk operation",
                required: Capability::EventsBookingCheckIn
            }
        ));

        let plan = planner
            .plan_bulk_operation(
                request
                    .with_capability(Capability::EventsBookingCheckIn)
                    .with_idempotency_key(
                        IdempotencyKey::new("events:check-in:v1").expect("valid key"),
                    ),
            )
            .expect("bulk plan");

        assert_eq!(plan.definition.id.as_str(), "bulk.events.check-in");
        assert!(plan.planned_job.idempotency_key.is_some());
        assert_eq!(plan.target_count, 12);
    }

    #[test]
    fn search_and_report_definitions_are_registry_ready() {
        let module = OpsModule::new();
        let manifest = module.manifest();

        assert_eq!(manifest.name, "ops");
        assert_eq!(manifest.config_namespace.as_deref(), Some("ops"));
        assert!(
            manifest
                .required_capabilities
                .contains(&Capability::AdminShellAccess)
        );
        assert!(
            manifest
                .optional_capabilities
                .contains(&Capability::CmsPagePublish)
        );
        assert!(
            manifest
                .module_dependencies
                .iter()
                .any(|dependency| dependency.module == "admin")
        );
        assert_eq!(manifest.migrations.len(), 3);
        assert_eq!(manifest.route_surfaces.len(), 3);
        assert_eq!(manifest.http_surfaces.len(), 3);
        assert_eq!(manifest.jobs.len(), 3);
        assert_eq!(manifest.event_subscriptions.len(), 3);
        assert_eq!(manifest.admin_resources.len(), 3);
        assert_eq!(manifest.report_definitions.len(), 1);
        assert_eq!(manifest.bulk_operations.len(), 2);
        assert!(
            manifest
                .core_service_dependencies
                .contains(&CoreServiceDependency::Storage)
        );
        assert!(
            manifest
                .extension_slots
                .iter()
                .any(|slot| slot.kind == ExtensionSlotKind::Job)
        );
        assert_eq!(
            module
                .install_migration_plan()
                .expect("ops migration plan")
                .ordered_steps()
                .len(),
            3
        );

        let mut registry = ServiceRegistry::new();
        module.register(&mut registry).expect("module register");
        assert_eq!(registry.services().count(), 5);
        assert_eq!(registry.modules().count(), 0);
    }
}
