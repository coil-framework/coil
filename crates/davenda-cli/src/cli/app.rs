use crate::CliModelError;
use crate::cli::args::{
    AssetsPublishInvocation, AuthBindingsInspectInvocation, AuthCheckInvocation,
    AuthListInvocation, AuthLookupInvocation, AuthPackageInspectInvocation,
    AuthPackageValidateInvocation, AuthTestModelInvocation, CacheInspectInvocation,
    CacheInvalidateInvocation, CacheWarmInvocation, CliInput, DevServerInvocation,
    JobsDeadLettersInvocation, JobsInFlightInvocation, JobsPromoteInvocation, JobsReadyInvocation,
    JobsRetryInvocation, JobsRunInvocation, JobsStatusInvocation, MigrateApplyInvocation,
    ModuleDisableInvocation, ModuleEnableInvocation, ModuleInspectInvocation,
    ModuleInstallInvocation, StorageInspectInvocation, TlsRenewInvocation,
    TlsValidateChallengeInvocation, parse,
};
use crate::cli::auth::AuthExplainResult;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::customer_app::{
    load_customer_app_context, load_official_modules, resolve_customer_app_root,
};
use crate::cli::error::CliRunError;
use crate::cli::import::{ImportCutoverInvocation, ImportRunInvocation};
use crate::cli::render::{render_auth_explain, render_command_report};
use crate::registry::CliRuntime;
use crate::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};
use davenda_app::{
    CustomerAppManifest, CustomerAppRuntimePlan, MigrationPlanEntry, MigrationPlanOwner,
    ReleaseDoctorSeverity,
};
use davenda_assets::{AssetDeliveryTarget, ContentFingerprint, FingerprintAlgorithm, RevisionId};
use davenda_auth::{
    AuthModelPackage, AuthModelPackageSelection, Capability, DavendaAuth, DefaultSubject,
    DefaultTuple, DefaultTupleUpdate, Entity, Namespace, Relation, configured_auth_model_package,
    default_auth_model_package, load_auth_model_package_at,
};
use davenda_cache::{CacheInstant, InvalidationSet, InvalidationTag};
use davenda_commerce::EntitlementKey;
use davenda_config::{DatabaseConfig, PlatformConfig, StorageClass};
use davenda_core::validate_module_capabilities;
use davenda_data::{
    CompiledStatement, CompiledTransaction, DataRuntime, DataValue, DomainWrite, MigrationPlan,
    MigrationRegistry, MutationAction, MutationSpec, PostgresDataClient, TransactionIsolation,
    TransactionPlan,
};
use davenda_import::{
    CutoverCheck, CutoverDnsRecordChange, CutoverExecutionJournal, CutoverPlan, CutoverStepRecord,
    CutoverSwitchExecution, CutoverTrafficTargetChange, ImportAuthMapping, ImportManifest,
    ImportModelError, PublicationMode, RollbackTrigger,
};
use davenda_jobs::{DeadLetterReason, JobFailureDisposition, JobInstant, QueueKind};
use davenda_memberships::{
    BillingInterval, MemberAccountId, MembershipTierId, SubscriptionId, SubscriptionStatus,
    TierVisibility,
};
use davenda_runtime::{
    BrowserInstant, CacheDisposition, EnvironmentSecretResolver, HandlerDefinition,
    HandlerResponse, HttpMethod, JobsHost, RequestExecutionError, RequestInput, RouteArea,
    RouteAuthGate, RouteDefinition, SecretResolver, SessionIssueRequest, StorageHost,
    WebhookObservationEvent, WebhookObservationSnapshot, WebhookObservationStatus,
};
use davenda_storage::{
    StorageDeliveryLocation, StoragePlanRequest, StoragePolicy, StoragePolicyOverride,
};
use davenda_tls::{
    CertificateId, CertificateStatus, CustomerAppId, Hostname, HostnameBinding, TlsInstant,
};
use reqwest::Url;
use reqwest::blocking::Client as BlockingHttpClient;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy as RedirectPolicy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliApplication {
    runtime: CliRuntime,
}

impl CliApplication {
    pub fn new(customer_app: impl Into<String>) -> Result<Self, CliModelError> {
        Ok(Self {
            runtime: CliRuntime::baseline(customer_app)?,
        })
    }

    pub fn runtime(&self) -> &CliRuntime {
        &self.runtime
    }
}

pub fn run_from_args(args: impl IntoIterator<Item = String>) -> Result<String, CliRunError> {
    let input = parse(args)?;
    match input {
        CliInput::Help => Ok(usage()),
        CliInput::DevServer { invocation } => {
            run_dev_server(&invocation)?;
            Ok(String::new())
        }
        CliInput::ConfigValidate {
            output_mode,
            invocation,
        } => {
            let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to load platform config from `{}`: {error}",
                    invocation.config_path.display()
                ))
            })?;

            let mut report = CommandReport::new(
                ["config", "validate"],
                format!(
                    "Validated effective platform configuration `{}`",
                    invocation.config_path.display()
                ),
            )
            .map_err(|error| {
                CliRunError::execution(format!("failed to build config report: {error}"))
            })?
            .with_columns([
                "app",
                "environment",
                "auth_package",
                "modules",
                "deployment",
            ])
            .map_err(|error| {
                CliRunError::execution(format!("failed to build config report: {error}"))
            })?;
            report.push_row(
                ReportRow::new()
                    .with_cell("app", config.app.name.clone())
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell("environment", environment_label(config.app.environment))
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell("auth_package", config.auth.package.clone())
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell(
                        "modules",
                        if config.modules.enabled.is_empty() {
                            "none".to_string()
                        } else {
                            config.modules.enabled.join(",")
                        },
                    )
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?
                    .with_cell(
                        "deployment",
                        storage_deployment_label(config.storage.deployment),
                    )
                    .map_err(|error| {
                        CliRunError::execution(format!("failed to build config report: {error}"))
                    })?,
            );

            render_command_report(&report, output_mode)
        }
        CliInput::AuthExplain {
            output_mode,
            invocation,
        } => {
            // Auth explain is deployment-configured and always goes through the live backend.
            let backend = LiveAuthExplainBackend::from_config_path(&invocation.config_path)?;
            let runtime = build_live_auth_runtime("auth explain")?;
            let explanation = runtime.block_on(async { backend.explain(&invocation).await })?;
            let result = AuthExplainResult {
                invocation,
                explanation,
            };
            render_auth_explain(&result, output_mode)
        }
        CliInput::AuthCheck {
            output_mode,
            invocation,
        } => {
            let report = run_auth_check(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AuthBindingsInspect {
            output_mode,
            invocation,
        } => {
            let report = run_auth_bindings_inspect(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AuthTestModel {
            output_mode,
            invocation,
        } => {
            let report = run_auth_test_model(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AuthList {
            output_mode,
            invocation,
        } => {
            let report = run_auth_list(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AuthLookup {
            output_mode,
            invocation,
        } => {
            let report = run_auth_lookup(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AuthPackageValidate {
            output_mode,
            invocation,
        } => {
            let report = run_auth_package_validate(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AuthPackageInspect {
            output_mode,
            invocation,
        } => {
            let report = run_auth_package_inspect(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ModuleList {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package =
                load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
            let composition = context
                .manifest
                .compose(&auth_package, &context.module_manifests)
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to compose customer app `{}` for module listing: {error}",
                        context.manifest.id
                    ))
                })?;
            let report = composition.module_list_report().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to render module list for `{}`: {error}",
                    config_path.display()
                ))
            })?;
            render_command_report(&report, output_mode)
        }
        CliInput::ModuleInspect {
            output_mode,
            invocation,
        } => {
            let report = run_module_inspect(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ModuleInstall {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_module_install(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ModuleEnable {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_module_enable(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ModuleDisable {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_module_disable(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::MigratePlan {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package =
                load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
            let migration_summary = context
                .manifest
                .migration_summary(auth_package, &context.modules);
            let report = migration_summary.command_report().map_err(|error| {
                CliRunError::execution(format!(
                    "failed to render migration plan for `{}`: {error}",
                    config_path.display()
                ))
            })?;
            render_command_report(&report, output_mode)
        }
        CliInput::MigrateApply {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_migrate_apply(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ReleaseDoctor {
            output_mode,
            config_path,
        } => {
            let report = run_release_doctor(&config_path)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ReleasePlan {
            output_mode,
            config_path,
        } => {
            let report = run_release_plan(&config_path)?;
            render_command_report(&report, output_mode)
        }
        CliInput::CacheWarm {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_cache_warm(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::CacheInspect {
            output_mode,
            invocation,
        } => {
            let report = run_cache_inspect(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::CacheInvalidate {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_cache_invalidate(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsStatus {
            output_mode,
            invocation,
        } => {
            let report = run_jobs_status(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsRun {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_jobs_run(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsReady {
            output_mode,
            invocation,
        } => {
            let report = run_jobs_ready(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsDeadLetters {
            output_mode,
            invocation,
        } => {
            let report = run_jobs_dead_letters(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsInFlight {
            output_mode,
            invocation,
        } => {
            let report = run_jobs_in_flight(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsRetry {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_jobs_retry(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::JobsPromote {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_jobs_promote(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::TlsStatus {
            output_mode,
            config_path,
        } => {
            let report = run_tls_status(&config_path)?;
            render_command_report(&report, output_mode)
        }
        CliInput::TlsValidateChallenge {
            output_mode,
            invocation,
        } => {
            let report = run_tls_validate_challenge(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::TlsRenew {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_tls_renew(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::StorageInspect {
            output_mode,
            invocation,
        } => {
            let report = run_storage_inspect(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::StorageVerify {
            output_mode,
            config_path,
            verify_policy,
        } => {
            let report = run_storage_verify(&config_path, verify_policy)?;
            render_command_report(&report, output_mode)
        }
        CliInput::AssetsPublish {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_assets_publish(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ImportRun {
            output_mode,
            dry_run,
            invocation,
        } => {
            let report = run_import_manifest(&invocation, dry_run)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ImportCutover {
            output_mode,
            dry_run,
            mut invocation,
        } => {
            invocation.dry_run = dry_run;
            let report = run_import_cutover(&invocation)?;
            render_command_report(&report, output_mode)
        }
    }
}

pub fn run_from_env() -> i32 {
    match run_from_args(std::env::args().skip(1)) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn usage() -> String {
    [
        "Usage:",
        "  platform dev server [--config <path>]",
        "  platform config validate [--config <path>] [--json]",
        "  platform auth check [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "  platform auth bindings inspect [--config <path>] [--capability <capability>] [--json]",
        "  platform auth test-model <spec-path> [--config <path>] [--json]",
        "  platform auth list [--config <path>] --subject <subject> --relation <relation> --namespace <namespace> [--json]",
        "  platform auth lookup [--config <path>] --resource <namespace:id> --relation <relation> --subject-namespace <namespace> [--json]",
        "  platform auth explain [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "  platform auth package validate [--config <path>] [--json]",
        "  platform auth package inspect [--config <path>] [--json]",
        "  platform module list [--config <path>] [--json]",
        "  platform module inspect <module> [--config <path>] [--json]",
        "  platform module install <module> [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform module enable <module> [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform module disable <module> [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform migrate plan [--config <path>] [--json]",
        "  platform migrate apply [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform release doctor [--config <path>] [--json]",
        "  platform release plan [--config <path>] [--json]",
        "  platform cache warm [--config <path>] --scope public --route <path> [--route <path> ...] [--dry-run] [--json]",
        "  platform cache inspect [--config <path>] --route <path> [--json]",
        "  platform cache invalidate [--config <path>] --tag <tag> [--tag <tag> ...] [--dry-run] --yes [--json]",
        "  platform jobs status [--config <path>] [--queue <name>] [--json]",
        "  platform jobs run [--config <path>] [--queue <name>] [--worker-id <id>] [--limit <n>] [--dry-run] [--json]",
        "  platform jobs ready [--config <path>] [--queue <name>] [--limit <n>] [--json]",
        "  platform jobs dead-letters [--config <path>] [--queue <name>] [--limit <n>] [--json]",
        "  platform jobs in-flight [--config <path>] [--queue <name>] [--worker-id <id>] [--limit <n>] [--json]",
        "  platform jobs retry <dead-letter-id> [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform jobs promote [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform tls status [--config <path>] [--json]",
        "  platform tls validate-challenge [--config <path>] [--json]",
        "  platform tls renew [--config <path>] --certificate <id> --replacement <id> [--dry-run] [--yes] [--json]",
        "  platform storage inspect [--config <path>] [--json]",
        "  platform storage verify [--config <path>] [--policy] [--json]",
        "  platform assets publish [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform import run <manifest-path> [--dry-run] [--json]",
        "  platform import cutover <manifest-path> [--apply] [--yes] [--legacy-freeze-confirmed] [--json]",
        "  platform import cutover <manifest-path> --switch --base-url <url> [--dry-run] [--dns-zone-id <zone> --dns-target <hostname> | --switch-zone-id <zone> --switch-resource-id <id> --switch-target <target>] [--yes] [--json]",
        "  platform import cutover <manifest-path> --observe --base-url <url> --yes [--json]",
        "  platform import cutover <manifest-path> --rollback --base-url <url> --reason <text> --yes [--json]",
        "",
        "Examples:",
        "  platform dev server --config config/platform.toml",
        "  platform config validate --config config/platform.toml",
        "  platform auth check --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform auth bindings inspect --config config/platform.toml --capability cms.page.publish",
        "  platform auth test-model config/auth-model.toml --config config/platform.toml",
        "  platform auth list --config config/platform.toml --subject user:alice --relation view --namespace page",
        "  platform auth lookup --config config/platform.toml --resource page:homepage --relation view --subject-namespace user",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform auth package validate --config config/platform.toml",
        "  platform auth package inspect --config config/platform.toml",
        "  platform module list --config config/platform.toml",
        "  platform module inspect cms --config config/platform.toml",
        "  platform module install media --config config/platform.toml --dry-run",
        "  platform module enable media --config config/platform.toml --dry-run",
        "  platform module disable media --config config/platform.toml --dry-run",
        "  platform migrate plan --config config/platform.toml",
        "  platform migrate apply --config config/platform.toml --dry-run",
        "  platform release doctor --config config/platform.toml",
        "  platform release plan --config config/platform.toml",
        "  platform cache warm --config config/platform.toml --scope public --route /en-GB/home",
        "  platform cache inspect --config config/platform.toml --route /en-GB/home",
        "  platform cache invalidate --config config/platform.toml --tag route:events.list --tag locale:en-GB --yes",
        "  platform jobs status --config config/platform.toml",
        "  platform jobs run --config config/platform.toml --worker-id worker-a --limit 25",
        "  platform jobs ready --config config/platform.toml --queue jobs.work --limit 25",
        "  platform jobs dead-letters --config config/platform.toml --queue jobs.dead-letter --limit 25",
        "  platform jobs in-flight --config config/platform.toml --queue jobs.work --worker-id worker-a --limit 25",
        "  platform jobs retry dead-letter:job-retry --config config/platform.toml --dry-run",
        "  platform jobs promote --config config/platform.toml --dry-run",
        "  platform tls status --config config/platform.toml",
        "  platform tls validate-challenge --config config/platform.toml",
        "  platform tls renew --config config/platform.toml --certificate cert-live --replacement cert-next --dry-run",
        "  platform storage inspect --config config/platform.toml",
        "  platform storage verify --config config/platform.toml --policy",
        "  platform assets publish --config apps/shoppr/platform.toml --dry-run",
        "  platform import run imports/wordpress-events.toml",
        "  platform import run imports/wordpress-events.toml --dry-run",
        "  platform import cutover imports/wordpress-events.toml",
        "  platform import cutover imports/wordpress-events.toml --apply --yes --legacy-freeze-confirmed",
        "  platform import cutover imports/wordpress-events.toml --switch --base-url https://shop.example.com --dns-zone-id zone_123 --dns-target davenda-origin.example.net --yes",
        "  platform import cutover imports/wordpress-events.toml --switch --base-url https://shop.example.com --switch-zone-id zone_123 --switch-resource-id lb-edge-1 --switch-target davenda-origin-pool --dry-run",
        "  platform import cutover imports/wordpress-events.toml --observe --base-url https://shop.example.com --yes",
        "  platform import cutover imports/wordpress-events.toml --rollback --base-url https://shop.example.com --reason \"edge routing failed\" --yes",
        "  platform import cutover imports/wordpress-events.toml --rollback --base-url https://shop.example.com --reason \"systemic auth failure\" --yes",
        "",
        "Environment:",
        "  DAVENDA_COOKIE_SECRET and DAVENDA_CSRF_SECRET are required for `dev server`",
        "  DATABASE_URL and OBJECT_STORE_URL are required by `config/platform.toml`",
    ]
    .join("\n")
}

#[derive(Debug)]
struct BuiltCustomerAppContext {
    app_root: PathBuf,
    manifest: CustomerAppManifest,
    runtime_plan: CustomerAppRuntimePlan,
}

#[derive(Debug)]
struct BuiltImportRuntimeContext {
    built: BuiltCustomerAppContext,
    storage_host: StorageHost,
}

#[derive(Clone)]
struct LiveImportAuthContext {
    auth: DavendaAuth<zanzibar::postgres::PostgresRebacEngine>,
    site_id: Option<String>,
    storefront_id: String,
    auth_package: AuthModelPackageSelection,
    auth_mapping: Option<ImportAuthMapping>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ImportAuthGrantScope {
    Site,
    Storefront,
}

impl ImportAuthGrantScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Storefront => "storefront",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedImportAuthGrant {
    scope: ImportAuthGrantScope,
    relation: Relation,
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AuthModelTestDocument {
    #[serde(rename = "case", default)]
    cases: Vec<AuthModelTestCaseDocument>,
}

#[derive(Debug, Deserialize)]
struct AuthModelTestCaseDocument {
    name: String,
    subject: String,
    capability: String,
    resource: String,
    expect: bool,
}

fn build_live_auth_backend(
    config_path: &Path,
    operation: &str,
) -> Result<
    (
        PlatformConfig,
        DavendaAuth<zanzibar::postgres::PostgresRebacEngine>,
        tokio::runtime::Runtime,
    ),
    CliRunError,
> {
    let runtime = build_live_auth_runtime(operation)?;
    let _runtime_guard = runtime.enter();
    let config = PlatformConfig::from_file(config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live {operation} backend: failed to load platform config from `{}`: {error}",
            config_path.display(),
        ))
    })?;
    let data = DataRuntime::from_config(&config.database).map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live {operation} backend: {error}"
        ))
    })?;
    let client = data.connect_lazy_postgres().map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live {operation} backend: {error}"
        ))
    })?;
    verify_live_data_client(&runtime, &client, operation)?;
    let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
    let auth = DavendaAuth::new(engine, config.auth.tenant_id);

    Ok((config, auth, runtime))
}

fn build_live_auth_runtime(operation: &str) -> Result<tokio::runtime::Runtime, CliRunError> {
    build_cli_async_runtime().map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live {operation} backend: {error}"
        ))
    })
}

fn build_cli_async_runtime() -> Result<tokio::runtime::Runtime, CliRunError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            CliRunError::execution(format!("failed to start the CLI async runtime: {error}"))
        })
}

fn build_dev_server_async_runtime() -> Result<tokio::runtime::Runtime, CliRunError> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("davenda-dev-server")
        .enable_all()
        .build()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to start the dev server async runtime: {error}"
            ))
        })
}

fn verify_live_data_client(
    runtime: &tokio::runtime::Runtime,
    client: &PostgresDataClient,
    operation: &str,
) -> Result<(), CliRunError> {
    runtime.block_on(client.ping()).map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live {operation} backend: {error}"
        ))
    })
}

fn live_jobs_state_unavailable_reason(
    built: &BuiltCustomerAppContext,
    runtime: &tokio::runtime::Runtime,
    guidance: &str,
) -> Result<Option<String>, CliRunError> {
    let _runtime_guard = runtime.enter();
    let Some(database_url) = std::env::var("DATABASE_URL").ok() else {
        return Ok(Some(format!(
            "live jobs coordinator state is unavailable for `{}`: set DATABASE_URL to {guidance}",
            built.manifest.id
        )));
    };

    let data_runtime = built
        .runtime_plan
        .runtime
        .data
        .with_resolved_connection_url(database_url);
    let client = data_runtime.connect_lazy_postgres().map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live jobs backend for `{}`: {error}",
            built.manifest.id
        ))
    })?;
    if let Err(error) = runtime.block_on(client.ping()) {
        return Ok(Some(format!(
            "live jobs coordinator state is unavailable for `{}`: DATABASE_URL could not initialize live state: {error}",
            built.manifest.id
        )));
    }

    Ok(None)
}

fn build_cli_jobs_host(
    built: &BuiltCustomerAppContext,
    node_id: &str,
    operation: &str,
) -> Result<(tokio::runtime::Runtime, JobsHost), CliRunError> {
    let runtime = build_cli_async_runtime()?;
    let _runtime_guard = runtime.enter();
    let host = built
        .runtime_plan
        .runtime
        .jobs_host(node_id)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build jobs {operation} host for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    Ok((runtime, host))
}

fn load_auth_package_from_app_root(
    app_root: &Path,
    package_name: &str,
) -> Result<davenda_auth::LoadedAuthModelPackage, CliRunError> {
    load_auth_model_package_at(package_name, app_root).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load auth package `{package_name}` from customer app `{}`: {error}",
            app_root.display()
        ))
    })
}

fn load_configured_auth_package(
    config_path: &Path,
) -> Result<davenda_auth::LoadedAuthModelPackage, CliRunError> {
    let config = PlatformConfig::from_file(config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load platform config from `{}`: {error}",
            config_path.display()
        ))
    })?;
    if config.auth.package == default_auth_model_package().manifest().name {
        return load_auth_package_from_app_root(Path::new("."), &config.auth.package);
    }

    let app_root = resolve_customer_app_root(config_path, &config.app.name)?;
    load_auth_package_from_app_root(&app_root, &config.auth.package)
}

fn load_live_auth_package(
    config_path: &Path,
    operation: &str,
) -> Result<davenda_auth::LoadedAuthModelPackage, CliRunError> {
    load_configured_auth_package(config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live {operation} backend: {error}"
        ))
    })
}

fn run_auth_check(invocation: &AuthCheckInvocation) -> Result<CommandReport, CliRunError> {
    let (_config, auth, runtime) = build_live_auth_backend(&invocation.config_path, "auth check")?;
    let package = load_live_auth_package(&invocation.config_path, "auth check")?;
    let binding = package
        .resolve_binding(invocation.capability, &invocation.resource)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve capability binding for auth check: {error}"
            ))
        })?;
    let allowed = runtime
        .block_on(async {
            auth.check_capability(
                &package,
                &invocation.subject,
                invocation.capability,
                &invocation.resource,
            )
            .await
        })
        .map_err(|error| {
            CliRunError::execution(format!("failed to execute auth check: {error}"))
        })?;

    let mut report = CommandReport::new(
        ["auth", "check"],
        format!(
            "Checked capability `{}` for subject `{}` on `{}`",
            invocation.capability,
            render_subject(&invocation.subject),
            render_entity(&invocation.resource)
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "subject",
        "capability",
        "resource",
        "result",
        "relation",
        "auth_package",
    ])
    .map_err(report_build_error)?;
    report = report.with_status(if allowed {
        ReportStatus::Ok
    } else {
        ReportStatus::Warning
    });
    report.push_row(
        ReportRow::new()
            .with_cell("subject", render_subject(&invocation.subject))
            .map_err(report_build_error)?
            .with_cell("capability", invocation.capability.to_string())
            .map_err(report_build_error)?
            .with_cell("resource", render_entity(&invocation.resource))
            .map_err(report_build_error)?
            .with_cell("result", if allowed { "allowed" } else { "denied" })
            .map_err(report_build_error)?
            .with_cell("relation", binding.relation.as_str())
            .map_err(report_build_error)?
            .with_cell("auth_package", package.manifest().name.clone())
            .map_err(report_build_error)?,
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.check.binding",
        format!(
            "capability `{}` resolves to relation `{}` for namespaces [{}]",
            invocation.capability,
            binding.relation.as_str(),
            binding
                .resource_namespaces
                .iter()
                .map(|namespace| namespace.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )?;
    Ok(report)
}

fn run_auth_bindings_inspect(
    invocation: &AuthBindingsInspectInvocation,
) -> Result<CommandReport, CliRunError> {
    let package = load_configured_auth_package(&invocation.config_path)?;
    let mut bindings = package
        .capability_bindings()
        .values()
        .filter(|binding| {
            invocation
                .capability
                .is_none_or(|capability| binding.capability == capability)
        })
        .cloned()
        .collect::<Vec<_>>();
    bindings.sort_by_key(|binding| binding.capability.as_str().to_string());

    let mut report = CommandReport::new(
        ["auth", "bindings", "inspect"],
        format!(
            "Inspected auth capability bindings for package `{}`",
            package.manifest().name
        ),
    )
    .map_err(report_build_error)?
    .with_columns(["capability", "relation", "namespaces", "auth_package"])
    .map_err(report_build_error)?;

    for binding in bindings {
        report.push_row(
            ReportRow::new()
                .with_cell("capability", binding.capability.to_string())
                .map_err(report_build_error)?
                .with_cell("relation", binding.relation.as_str())
                .map_err(report_build_error)?
                .with_cell(
                    "namespaces",
                    binding
                        .resource_namespaces
                        .iter()
                        .map(|namespace| namespace.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .map_err(report_build_error)?
                .with_cell("auth_package", package.manifest().name.clone())
                .map_err(report_build_error)?,
        );
    }

    let binding_count = report.rows.len();
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.bindings.inspect",
        format!(
            "package={} binding_count={} capability_binding_version={}",
            package.manifest().name,
            binding_count,
            package.manifest().capability_binding_version
        ),
    )?;
    Ok(report)
}

fn run_auth_test_model(invocation: &AuthTestModelInvocation) -> Result<CommandReport, CliRunError> {
    let document = load_auth_model_test_document(&invocation.spec_path)?;
    let (_config, auth, runtime) =
        build_live_auth_backend(&invocation.config_path, "auth test-model")?;
    let package = load_live_auth_package(&invocation.config_path, "auth test-model")?;
    let mut report = CommandReport::new(
        ["auth", "test-model"],
        format!(
            "Ran auth model test cases from `{}`",
            invocation.spec_path.display()
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "case",
        "subject",
        "capability",
        "resource",
        "expected",
        "actual",
        "result",
    ])
    .map_err(report_build_error)?;
    let mut failed_cases = Vec::new();

    for case in document.cases {
        let subject = parse_auth_subject_spec(&case.subject, "subject")?;
        let capability = parse_auth_capability_spec(&case.capability, "capability")?;
        let resource = parse_auth_entity_spec(&case.resource, "resource")?;
        let actual = runtime
            .block_on(async {
                auth.check_capability(&package, &subject, capability, &resource)
                    .await
            })
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to execute auth test case `{}`: {error}",
                    case.name
                ))
            })?;
        let passed = actual == case.expect;
        if !passed {
            failed_cases.push(case.name.clone());
        }
        report.push_row(
            ReportRow::new()
                .with_cell("case", case.name.clone())
                .map_err(report_build_error)?
                .with_cell("subject", render_subject(&subject))
                .map_err(report_build_error)?
                .with_cell("capability", capability.to_string())
                .map_err(report_build_error)?
                .with_cell("resource", render_entity(&resource))
                .map_err(report_build_error)?
                .with_cell("expected", if case.expect { "allowed" } else { "denied" })
                .map_err(report_build_error)?
                .with_cell("actual", if actual { "allowed" } else { "denied" })
                .map_err(report_build_error)?
                .with_cell("result", if passed { "pass" } else { "fail" })
                .map_err(report_build_error)?,
        );
    }

    if failed_cases.is_empty() {
        report = report.with_status(ReportStatus::Ok);
    } else {
        report = report.with_status(ReportStatus::Unsafe);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Error,
            "auth.test_model.failed",
            format!("failing test cases: {}", failed_cases.join(", ")),
        )?;
    }
    let case_count = report.rows.len();
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.test_model.summary",
        format!(
            "package={} cases={} failures={}",
            package.manifest().name,
            case_count,
            failed_cases.len()
        ),
    )?;
    Ok(report)
}

fn run_auth_list(invocation: &AuthListInvocation) -> Result<CommandReport, CliRunError> {
    let (_config, auth, runtime) = build_live_auth_backend(&invocation.config_path, "auth list")?;
    let package = load_live_auth_package(&invocation.config_path, "auth list")?;
    let mut object_ids = runtime
        .block_on(async {
            auth.list_objects(
                &invocation.subject,
                invocation.relation,
                invocation.namespace,
            )
            .await
        })
        .map_err(|error| CliRunError::execution(format!("failed to execute auth list: {error}")))?;
    object_ids.sort();

    let mut report = CommandReport::new(
        ["auth", "list"],
        format!(
            "Listed `{}` objects reachable for `{}` via relation `{}`",
            invocation.namespace,
            render_subject(&invocation.subject),
            invocation.relation
        ),
    )
    .map_err(report_build_error)?
    .with_columns(["subject", "relation", "namespace", "object", "auth_package"])
    .map_err(report_build_error)?;

    for object_id in &object_ids {
        report.push_row(
            ReportRow::new()
                .with_cell("subject", render_subject(&invocation.subject))
                .map_err(report_build_error)?
                .with_cell("relation", invocation.relation.as_str())
                .map_err(report_build_error)?
                .with_cell("namespace", invocation.namespace.as_str())
                .map_err(report_build_error)?
                .with_cell(
                    "object",
                    render_namespace_identifier(invocation.namespace, object_id),
                )
                .map_err(report_build_error)?
                .with_cell("auth_package", package.manifest().name.clone())
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.list.results",
        format!(
            "package={} subject={} relation={} namespace={} count={}",
            package.manifest().name,
            render_subject(&invocation.subject),
            invocation.relation,
            invocation.namespace,
            object_ids.len()
        ),
    )?;
    Ok(report)
}

fn run_auth_lookup(invocation: &AuthLookupInvocation) -> Result<CommandReport, CliRunError> {
    let (_config, auth, runtime) = build_live_auth_backend(&invocation.config_path, "auth lookup")?;
    let package = load_live_auth_package(&invocation.config_path, "auth lookup")?;
    let mut subject_ids = runtime
        .block_on(async {
            auth.list_subject_ids(
                &invocation.resource,
                invocation.relation,
                invocation.subject_namespace,
            )
            .await
        })
        .map_err(|error| {
            CliRunError::execution(format!("failed to execute auth lookup: {error}"))
        })?;
    subject_ids.sort();

    let mut report = CommandReport::new(
        ["auth", "lookup"],
        format!(
            "Looked up `{}` subjects for `{}` via relation `{}`",
            invocation.subject_namespace,
            render_entity(&invocation.resource),
            invocation.relation
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "resource",
        "relation",
        "subject_namespace",
        "subject",
        "auth_package",
    ])
    .map_err(report_build_error)?;

    for subject_id in &subject_ids {
        report.push_row(
            ReportRow::new()
                .with_cell("resource", render_entity(&invocation.resource))
                .map_err(report_build_error)?
                .with_cell("relation", invocation.relation.as_str())
                .map_err(report_build_error)?
                .with_cell("subject_namespace", invocation.subject_namespace.as_str())
                .map_err(report_build_error)?
                .with_cell(
                    "subject",
                    render_namespace_identifier(invocation.subject_namespace, subject_id),
                )
                .map_err(report_build_error)?
                .with_cell("auth_package", package.manifest().name.clone())
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.lookup.results",
        format!(
            "package={} resource={} relation={} subject_namespace={} count={}",
            package.manifest().name,
            render_entity(&invocation.resource),
            invocation.relation,
            invocation.subject_namespace,
            subject_ids.len()
        ),
    )?;
    Ok(report)
}

fn run_auth_package_validate(
    invocation: &AuthPackageValidateInvocation,
) -> Result<CommandReport, CliRunError> {
    let context = load_customer_app_context(&invocation.config_path)?;
    let auth_package =
        load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
    let package_manifest = auth_package.manifest().clone();
    let mut report = CommandReport::new(
        ["auth", "package", "validate"],
        format!(
            "Validated auth package `{}` against customer app `{}`",
            package_manifest.name, context.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "module",
        "status",
        "required_capabilities",
        "optional_capabilities",
        "detail",
    ])
    .map_err(report_build_error)?;

    for installed in &context.manifest.modules {
        let manifest = context
            .module_manifests
            .iter()
            .find(|candidate| candidate.name == installed.id.as_str())
            .ok_or_else(|| {
                CliRunError::execution(format!(
                    "customer app `{}` declares unknown module `{}`",
                    context.manifest.id, installed.id
                ))
            })?;
        let result = validate_module_capabilities(&auth_package, manifest);
        let status = if result.is_ok() { "valid" } else { "invalid" };
        if result.is_err() {
            report = report.with_status(ReportStatus::Unsafe);
        }
        report.push_row(
            ReportRow::new()
                .with_cell("module", manifest.name.clone())
                .map_err(report_build_error)?
                .with_cell("status", status)
                .map_err(report_build_error)?
                .with_cell(
                    "required_capabilities",
                    manifest.required_capabilities.len().to_string(),
                )
                .map_err(report_build_error)?
                .with_cell(
                    "optional_capabilities",
                    manifest.optional_capabilities.len().to_string(),
                )
                .map_err(report_build_error)?
                .with_cell(
                    "detail",
                    result
                        .map(|_| {
                            format!(
                                "{} bound capability contracts validated",
                                manifest.required_capabilities.len()
                            )
                        })
                        .unwrap_or_else(|error| error.to_string()),
                )
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.package.manifest",
        format!(
            "package={} version={} mode={} storage_schema_version={} model_version={} capability_binding_version={} bindings={}",
            package_manifest.name,
            package_manifest.version,
            package_manifest.mode,
            package_manifest.storage_schema_version,
            package_manifest.model_version,
            package_manifest.capability_binding_version,
            auth_package.capability_bindings().len()
        ),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.package.modules",
        format!(
            "validated {} installed module(s) for customer app `{}`",
            context.manifest.modules.len(),
            context.manifest.id
        ),
    )?;

    Ok(report)
}

fn run_auth_package_inspect(
    invocation: &AuthPackageInspectInvocation,
) -> Result<CommandReport, CliRunError> {
    let context = load_customer_app_context(&invocation.config_path)?;
    let auth_package =
        load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
    let package_manifest = auth_package.manifest().clone();
    let imports = if package_manifest.imports.is_empty() {
        "none".to_string()
    } else {
        package_manifest.imports.join(", ")
    };

    let mut report = CommandReport::new(
        ["auth", "package", "inspect"],
        format!(
            "Inspected auth package `{}` for customer app `{}`",
            package_manifest.name, context.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "package",
        "version",
        "mode",
        "model_version",
        "storage_schema_version",
        "binding_version",
        "bindings",
        "imports",
    ])
    .map_err(report_build_error)?;

    report.push_row(
        ReportRow::new()
            .with_cell("package", package_manifest.name.clone())
            .map_err(report_build_error)?
            .with_cell("version", package_manifest.version.to_string())
            .map_err(report_build_error)?
            .with_cell("mode", package_manifest.mode.to_string())
            .map_err(report_build_error)?
            .with_cell("model_version", package_manifest.model_version.to_string())
            .map_err(report_build_error)?
            .with_cell(
                "storage_schema_version",
                package_manifest.storage_schema_version.to_string(),
            )
            .map_err(report_build_error)?
            .with_cell(
                "binding_version",
                package_manifest.capability_binding_version.to_string(),
            )
            .map_err(report_build_error)?
            .with_cell(
                "bindings",
                auth_package.capability_bindings().len().to_string(),
            )
            .map_err(report_build_error)?
            .with_cell("imports", imports)
            .map_err(report_build_error)?,
    );

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.package.customer_app",
        format!(
            "customer_app={} installed_modules={}",
            context.manifest.id,
            context.manifest.modules.len()
        ),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "auth.package.runtime_source",
        format!(
            "loaded auth package implementation `{}` for customer app `{}` with {} effective capability bindings",
            package_manifest.name,
            context.manifest.id,
            auth_package.capability_bindings().len()
        ),
    )?;
    Ok(report)
}

fn run_module_inspect(invocation: &ModuleInspectInvocation) -> Result<CommandReport, CliRunError> {
    let context = load_customer_app_context(&invocation.config_path)?;
    let auth_package =
        load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
    validate_supported_official_module(&invocation.module)?;
    let installed_spec = context
        .manifest
        .modules
        .iter()
        .find(|spec| spec.id.as_str() == invocation.module);
    let module_manifest = context
        .module_manifests
        .iter()
        .find(|candidate| candidate.name == invocation.module)
        .cloned()
        .unwrap_or(supported_official_module_manifest(&invocation.module)?);
    let installed = installed_spec.is_some();
    let auth_validation = validate_module_capabilities(&auth_package, &module_manifest);
    let installed_modules = context
        .manifest
        .modules
        .iter()
        .map(|module| module.id.to_string())
        .collect::<Vec<_>>();
    let missing_required_dependencies = module_manifest
        .module_dependencies
        .iter()
        .filter(|dependency| {
            matches!(
                dependency.kind,
                davenda_core::ModuleDependencyKind::Required
            ) && !installed_modules.contains(&dependency.module)
        })
        .map(|dependency| dependency.module.clone())
        .collect::<Vec<_>>();

    let mut report = CommandReport::new(
        ["module", "inspect"],
        format!(
            "Inspected module `{}` for customer app `{}`",
            module_manifest.name, context.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns(["category", "item", "status", "detail"])
    .map_err(report_build_error)?;

    if installed && (auth_validation.is_err() || !missing_required_dependencies.is_empty()) {
        report = report.with_status(ReportStatus::Unsafe);
    } else if auth_validation.is_err()
        || !missing_required_dependencies.is_empty()
        || !installed
        || installed_spec
            .and_then(|spec| spec.version_req.as_ref())
            .is_none()
    {
        report = report.with_status(ReportStatus::Warning);
    }

    let version = installed_spec
        .and_then(|spec| spec.version_req.clone())
        .unwrap_or_else(|| {
            if installed {
                "unpinned".to_string()
            } else {
                "not_installed".to_string()
            }
        });
    let config_namespace = module_manifest
        .config_namespace
        .clone()
        .unwrap_or_else(|| "none".to_string());

    report.push_row(
        ReportRow::new()
            .with_cell("category", "installation")
            .map_err(report_build_error)?
            .with_cell("item", "module")
            .map_err(report_build_error)?
            .with_cell("status", if installed { "installed" } else { "available" })
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                format!("version={version} config_namespace={config_namespace}"),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "auth")
            .map_err(report_build_error)?
            .with_cell("item", "required_capabilities")
            .map_err(report_build_error)?
            .with_cell(
                "status",
                if auth_validation.is_ok() {
                    "valid"
                } else {
                    "invalid"
                },
            )
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .required_capabilities
                        .iter()
                        .map(|capability| capability.as_str().to_string()),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "auth")
            .map_err(report_build_error)?
            .with_cell("item", "optional_capabilities")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .optional_capabilities
                        .iter()
                        .map(|capability| capability.as_str().to_string()),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "auth")
            .map_err(report_build_error)?
            .with_cell("item", "capability_contracts")
            .map_err(report_build_error)?
            .with_cell(
                "status",
                if auth_validation.is_ok() {
                    "valid"
                } else {
                    "invalid"
                },
            )
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(module_manifest.capability_contracts.iter().map(|contract| {
                    format!(
                        "{}:{} [{}]",
                        if contract.required {
                            "required"
                        } else {
                            "optional"
                        },
                        contract.capability.as_str(),
                        summarize_items(contract.resource_kinds.iter().cloned())
                    )
                })),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "dependencies")
            .map_err(report_build_error)?
            .with_cell("item", "modules")
            .map_err(report_build_error)?
            .with_cell(
                "status",
                if missing_required_dependencies.is_empty() {
                    "satisfied"
                } else {
                    "missing"
                },
            )
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .module_dependencies
                        .iter()
                        .map(|dependency| {
                            format!(
                                "{}:{}",
                                format!("{:?}", dependency.kind).to_lowercase(),
                                dependency.module
                            )
                        }),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "dependencies")
            .map_err(report_build_error)?
            .with_cell("item", "core_services")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .core_service_dependencies
                        .iter()
                        .map(|dependency| format!("{dependency:?}").to_lowercase()),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "lifecycle")
            .map_err(report_build_error)?
            .with_cell("item", "migrations")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(module_manifest.migrations.iter().map(|migration| {
                    format!(
                        "{}:{} ({})",
                        migration.owner, migration.order, migration.description
                    )
                })),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "runtime")
            .map_err(report_build_error)?
            .with_cell("item", "routes")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(module_manifest.route_surfaces.iter().map(|route| {
                    let capability = route
                        .capability
                        .map(|capability| capability.as_str().to_string())
                        .unwrap_or_else(|| "public".to_string());
                    format!(
                        "{}:{:?}:{} [{}]",
                        route.name, route.kind, route.path, capability
                    )
                    .to_lowercase()
                })),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "runtime")
            .map_err(report_build_error)?
            .with_cell("item", "jobs")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(module_manifest.jobs.iter().map(|job| {
                    format!(
                        "{}:{:?}:{}",
                        job.name,
                        job.trigger,
                        if job.idempotent {
                            "idempotent"
                        } else {
                            "non_idempotent"
                        }
                    )
                    .to_lowercase()
                })),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "runtime")
            .map_err(report_build_error)?
            .with_cell("item", "subscriptions")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .event_subscriptions
                        .iter()
                        .map(|subscription| match &subscription.job {
                            Some(job) => format!("{} -> {}", subscription.event, job),
                            None => subscription.event.clone(),
                        }),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "runtime")
            .map_err(report_build_error)?
            .with_cell("item", "admin_resources")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(module_manifest.admin_resources.iter().map(|resource| {
                    format!(
                        "{}:{}:{}",
                        resource.id,
                        resource.route,
                        resource.required_capability.as_str()
                    )
                })),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "runtime")
            .map_err(report_build_error)?
            .with_cell("item", "extension_slots")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .extension_slots
                        .iter()
                        .map(|slot| format!("{}:{:?}", slot.surface, slot.kind).to_lowercase()),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "operations")
            .map_err(report_build_error)?
            .with_cell("item", "search")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .search_contributions
                        .iter()
                        .map(|contribution| contribution.id.clone()),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "operations")
            .map_err(report_build_error)?
            .with_cell("item", "reports")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .report_definitions
                        .iter()
                        .map(|definition| definition.id.clone()),
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("category", "operations")
            .map_err(report_build_error)?
            .with_cell("item", "bulk_operations")
            .map_err(report_build_error)?
            .with_cell("status", "declared")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                summarize_items(
                    module_manifest
                        .bulk_operations
                        .iter()
                        .map(|operation| operation.id.clone()),
                ),
            )
            .map_err(report_build_error)?,
    );

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "module.inspect.summary",
        format!(
            "module={} installed={} version={} required_capabilities={} optional_capabilities={} routes={} jobs={} subscriptions={} extension_slots={}",
            module_manifest.name,
            installed,
            version,
            module_manifest.required_capabilities.len(),
            module_manifest.optional_capabilities.len(),
            module_manifest.route_surfaces.len(),
            module_manifest.jobs.len(),
            module_manifest.event_subscriptions.len(),
            module_manifest.extension_slots.len()
        ),
    )?;
    if !installed {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "module.installation.available",
            format!(
                "official module `{}` is available for customer app `{}` but is not currently installed",
                module_manifest.name, context.manifest.id
            ),
        )?;
    } else if installed_spec
        .and_then(|spec| spec.version_req.as_ref())
        .is_none()
    {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "module.version.unpinned",
            format!(
                "official module `{}` is not version pinned in the customer app manifest",
                module_manifest.name
            ),
        )?;
    }
    if let Err(error) = auth_validation {
        push_report_diagnostic(
            &mut report,
            if installed {
                DiagnosticSeverity::Error
            } else {
                DiagnosticSeverity::Warning
            },
            "module.auth.invalid",
            error.to_string(),
        )?;
    }
    if !missing_required_dependencies.is_empty() {
        push_report_diagnostic(
            &mut report,
            if installed {
                DiagnosticSeverity::Error
            } else {
                DiagnosticSeverity::Warning
            },
            "module.dependencies.missing",
            format!(
                "required module dependencies are missing: {}",
                missing_required_dependencies.join(", ")
            ),
        )?;
    }

    Ok(report)
}

fn run_module_enable(
    invocation: &ModuleEnableInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    run_module_state_change(
        &invocation.config_path,
        &invocation.module,
        true,
        dry_run,
        invocation.confirmed,
        "enable",
        "enabled",
        "enabled",
    )
}

fn run_module_install(
    invocation: &ModuleInstallInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    run_module_state_change(
        &invocation.config_path,
        &invocation.module,
        true,
        dry_run,
        invocation.confirmed,
        "install",
        "installed",
        "installed",
    )
}

fn run_module_disable(
    invocation: &ModuleDisableInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    run_module_state_change(
        &invocation.config_path,
        &invocation.module,
        false,
        dry_run,
        invocation.confirmed,
        "disable",
        "disabled",
        "disabled",
    )
}

fn run_module_state_change(
    config_path: &Path,
    module: &str,
    enable: bool,
    dry_run: bool,
    confirmed: bool,
    command_action: &str,
    past_tense: &str,
    state_label: &str,
) -> Result<CommandReport, CliRunError> {
    validate_supported_official_module(module)?;

    let context = load_customer_app_context(config_path)?;
    let config_currently_enabled = context
        .config
        .modules
        .enabled
        .iter()
        .any(|candidate| candidate == module);
    let manifest_currently_enabled = context
        .manifest
        .modules
        .iter()
        .any(|candidate| candidate.id.as_str() == module);

    if config_currently_enabled != manifest_currently_enabled {
        return Err(CliRunError::execution(format!(
            "customer app `{}` has drifted module state for `{module}` between `{}` and `{}`",
            context.manifest.id,
            config_path.display(),
            context.app_root.join("app.toml").display()
        )));
    }

    if config_currently_enabled == enable {
        return build_module_state_change_noop_report(
            &context.manifest.id.to_string(),
            config_path,
            &context.app_root.join("app.toml"),
            module,
            command_action,
            state_label,
        );
    }

    if !dry_run && !confirmed {
        return Err(CliRunError::usage(format!(
            "`module {command_action}` requires `--yes` unless `--dry-run` is used"
        )));
    }

    let manifest_path = context.app_root.join("app.toml");
    let config_input = fs::read_to_string(config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to read platform config `{}`: {error}",
            config_path.display()
        ))
    })?;
    let manifest_input = fs::read_to_string(&manifest_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to read customer app manifest `{}`: {error}",
            manifest_path.display()
        ))
    })?;

    let mut config_document = parse_toml_document(config_path, &config_input)?;
    let mut manifest_document = parse_toml_document(&manifest_path, &manifest_input)?;
    update_enabled_modules_document(&mut config_document, module, enable).map_err(|message| {
        CliRunError::execution(format!(
            "failed to update modules.enabled in `{}` during module {command_action}: {message}",
            config_path.display()
        ))
    })?;
    update_enabled_modules_document(&mut manifest_document, module, enable).map_err(|message| {
        CliRunError::execution(format!(
            "failed to update modules.enabled in `{}` during module {command_action}: {message}",
            manifest_path.display()
        ))
    })?;

    let rendered_config = render_toml_document(config_path, &config_document)?;
    let rendered_manifest = render_toml_document(&manifest_path, &manifest_document)?;
    let updated_config = PlatformConfig::from_toml_str(&rendered_config).map_err(|error| {
        CliRunError::execution(format!(
            "updated platform config `{}` is invalid after module {command_action}: {error}",
            config_path.display()
        ))
    })?;
    let updated_manifest =
        CustomerAppManifest::from_toml_str(&rendered_manifest).map_err(|error| {
            CliRunError::execution(format!(
                "updated customer app manifest `{}` is invalid after module {command_action}: {error}",
                manifest_path.display()
            ))
        })?;
    updated_manifest
        .validate_runtime_config_alignment(&updated_config)
        .map_err(|error| {
            CliRunError::execution(format!(
                "module {command_action} would leave `{}` and `{}` out of alignment: {error}",
                config_path.display(),
                manifest_path.display()
            ))
        })?;
    let updated_modules = load_official_modules(&updated_config)?;
    let updated_module_manifests = updated_modules
        .iter()
        .map(|installed| installed.manifest().clone())
        .collect::<Vec<_>>();
    let auth_package =
        load_auth_package_from_app_root(&context.app_root, &updated_config.auth.package)?;
    updated_manifest
        .compose(&auth_package, &updated_module_manifests)
        .map_err(|error| {
            CliRunError::execution(format!(
                "module {command_action} would leave customer app `{}` with an invalid module composition: {error}",
                updated_manifest.id
            ))
        })?;

    if !dry_run {
        fs::write(config_path, &rendered_config).map_err(|error| {
            CliRunError::execution(format!(
                "failed to write platform config `{}`: {error}",
                config_path.display()
            ))
        })?;
        fs::write(&manifest_path, &rendered_manifest).map_err(|error| {
            CliRunError::execution(format!(
                "failed to write customer app manifest `{}`: {error}",
                manifest_path.display()
            ))
        })?;
    }

    let mut report = CommandReport::new(
        ["module", command_action],
        if dry_run {
            format!(
                "Planned module `{module}` to be {past_tense} for customer app `{}`",
                updated_manifest.id
            )
        } else {
            format!(
                "Module `{module}` {past_tense} for customer app `{}`",
                updated_manifest.id
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns(["target", "status", "detail"])
    .map_err(report_build_error)?;

    let change_status = if dry_run { "planned" } else { past_tense };
    for (target, path) in [
        ("platform_config", config_path),
        ("customer_app_manifest", manifest_path.as_path()),
    ] {
        report.push_row(
            ReportRow::new()
                .with_cell("target", target)
                .map_err(report_build_error)?
                .with_cell("status", change_status)
                .map_err(report_build_error)?
                .with_cell("detail", path.display().to_string())
                .map_err(report_build_error)?,
        );
    }
    report.push_row(
        ReportRow::new()
            .with_cell("target", "runtime_composition")
            .map_err(report_build_error)?
            .with_cell("status", "validated")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                format!(
                    "enabled_modules={}",
                    updated_config.modules.enabled.join(", ")
                ),
            )
            .map_err(report_build_error)?,
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "module.state_change",
        format!(
            "module `{module}` {past_tense}; app=`{}` dry_run={dry_run}",
            updated_manifest.id
        ),
    )?;

    Ok(report)
}

fn build_module_state_change_noop_report(
    app_id: &str,
    config_path: &Path,
    manifest_path: &Path,
    module: &str,
    command_action: &str,
    state_label: &str,
) -> Result<CommandReport, CliRunError> {
    let mut report = CommandReport::new(
        ["module", command_action],
        format!("Module `{module}` is already {state_label} for customer app `{app_id}`"),
    )
    .map_err(report_build_error)?
    .with_columns(["target", "status", "detail"])
    .map_err(report_build_error)?
    .with_status(ReportStatus::Warning);

    for (target, path) in [
        ("platform_config", config_path),
        ("customer_app_manifest", manifest_path),
    ] {
        report.push_row(
            ReportRow::new()
                .with_cell("target", target)
                .map_err(report_build_error)?
                .with_cell("status", "unchanged")
                .map_err(report_build_error)?
                .with_cell("detail", path.display().to_string())
                .map_err(report_build_error)?,
        );
    }
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "module.state_change.noop",
        format!("module `{module}` is already {state_label}"),
    )?;

    Ok(report)
}

fn validate_supported_official_module(module: &str) -> Result<(), CliRunError> {
    const SUPPORTED_OFFICIAL_MODULES: &[&str] = &[
        "admin",
        "cms",
        "commerce",
        "commerce-payments-stripe",
        "events",
        "media",
        "memberships",
        "ops",
    ];

    if SUPPORTED_OFFICIAL_MODULES.contains(&module) {
        Ok(())
    } else {
        Err(CliRunError::execution(format!(
            "unsupported official module `{module}`; expected one of: {}",
            SUPPORTED_OFFICIAL_MODULES.join(", ")
        )))
    }
}

fn supported_official_module_manifest(
    module: &str,
) -> Result<davenda_core::ModuleManifest, CliRunError> {
    let module: Box<dyn davenda_core::PlatformModule> = match module {
        "admin" => Box::new(davenda_admin::AdminModule::new()),
        "cms" => Box::new(davenda_cms::CmsModule::new()),
        "commerce" => Box::new(davenda_commerce::CommerceModule::new()),
        "commerce-payments-stripe" => {
            Box::new(davenda_commerce::CommercePaymentsStripeModule::new())
        }
        "events" => Box::new(davenda_events::EventsModule::new()),
        "media" => Box::new(davenda_media::MediaModule::new()),
        "memberships" => Box::new(davenda_memberships::MembershipsModule::new()),
        "ops" => Box::new(davenda_ops::OpsModule::new()),
        other => {
            return Err(CliRunError::execution(format!(
                "unsupported official module `{other}`"
            )));
        }
    };
    Ok(module.manifest().clone())
}

fn parse_toml_document(path: &Path, input: &str) -> Result<toml::Value, CliRunError> {
    toml::from_str(input).map_err(|error| {
        CliRunError::execution(format!(
            "failed to parse TOML document `{}`: {error}",
            path.display()
        ))
    })
}

fn render_toml_document(path: &Path, document: &toml::Value) -> Result<String, CliRunError> {
    toml::to_string_pretty(document).map_err(|error| {
        CliRunError::execution(format!(
            "failed to render TOML document `{}`: {error}",
            path.display()
        ))
    })
}

fn update_enabled_modules_document(
    document: &mut toml::Value,
    module: &str,
    enable: bool,
) -> Result<(), String> {
    let table = document
        .as_table_mut()
        .ok_or_else(|| "document root must be a TOML table".to_string())?;
    let modules_value = table
        .entry("modules".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    let modules_table = modules_value
        .as_table_mut()
        .ok_or_else(|| "`modules` must be a TOML table".to_string())?;
    let enabled_value = modules_table
        .entry("enabled".to_string())
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    let enabled_array = enabled_value
        .as_array_mut()
        .ok_or_else(|| "`modules.enabled` must be an array".to_string())?;

    let mut modules = enabled_array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| "`modules.enabled` must contain only strings".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    modules.retain(|candidate| candidate != module);
    if enable {
        modules.push(module.to_string());
    }
    *enabled_array = modules.into_iter().map(toml::Value::String).collect();
    Ok(())
}

fn run_release_doctor(config_path: &Path) -> Result<CommandReport, CliRunError> {
    let context = load_customer_app_context(config_path)?;
    let auth_package =
        load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
    context
        .manifest
        .release_doctor_with_extensions(
            &auth_package,
            &context.module_manifests,
            &[],
            Some(&context.config),
        )
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build release doctor report for `{}`: {error}",
                config_path.display()
            ))
        })?
        .command_report()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to render release doctor report for `{}`: {error}",
                config_path.display()
            ))
        })
}

fn run_release_plan(config_path: &Path) -> Result<CommandReport, CliRunError> {
    let context = load_customer_app_context(config_path)?;
    let auth_package =
        load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
    let migration_summary = context
        .manifest
        .migration_summary(auth_package, &context.modules);
    let doctor_auth_package =
        load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?;
    let doctor = context
        .manifest
        .release_doctor_with_extensions(
            &doctor_auth_package,
            &context.module_manifests,
            &[],
            Some(&context.config),
        )
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build release plan for `{}`: {error}",
                config_path.display()
            ))
        })?;

    let mut report = CommandReport::new(
        ["release", "plan"],
        format!(
            "Composed release plan for customer app `{}`",
            context.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns(["phase", "owner", "status", "detail"])
    .map_err(report_build_error)?;

    for entry in migration_summary.entries() {
        report.push_row(
            ReportRow::new()
                .with_cell("phase", "migration")
                .map_err(report_build_error)?
                .with_cell("owner", release_plan_owner_label(&entry.owner))
                .map_err(report_build_error)?
                .with_cell(
                    "status",
                    if entry.online_safe {
                        "online_safe"
                    } else {
                        "manual_review"
                    },
                )
                .map_err(report_build_error)?
                .with_cell(
                    "detail",
                    match &entry.step_id {
                        Some(step_id) => format!("{step_id}: {}", entry.description),
                        None => entry.description.clone(),
                    },
                )
                .map_err(report_build_error)?,
        );
    }

    for finding in &doctor.findings {
        report.push_row(
            ReportRow::new()
                .with_cell("phase", "compatibility")
                .map_err(report_build_error)?
                .with_cell("owner", "release_doctor")
                .map_err(report_build_error)?
                .with_cell("status", release_plan_severity_label(finding.severity))
                .map_err(report_build_error)?
                .with_cell("detail", format!("{}: {}", finding.code, finding.message))
                .map_err(report_build_error)?,
        );
    }

    report = report.with_status(
        if doctor
            .findings
            .iter()
            .any(|finding| finding.severity == ReleaseDoctorSeverity::Blocking)
        {
            ReportStatus::Unsafe
        } else if doctor
            .findings
            .iter()
            .any(|finding| finding.severity == ReleaseDoctorSeverity::Warning)
            || migration_summary
                .entries()
                .iter()
                .any(|entry| !entry.online_safe)
        {
            ReportStatus::Warning
        } else {
            ReportStatus::Ok
        },
    );

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "release.plan.summary",
        format!(
            "migrations={} compatibility_findings={} blocking={} warnings={}",
            migration_summary.entries().len(),
            doctor.findings.len(),
            doctor
                .findings
                .iter()
                .filter(|finding| finding.severity == ReleaseDoctorSeverity::Blocking)
                .count(),
            doctor
                .findings
                .iter()
                .filter(|finding| finding.severity == ReleaseDoctorSeverity::Warning)
                .count()
        ),
    )?;

    Ok(report)
}

fn run_migrate_apply(
    invocation: &MigrateApplyInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    if !dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`migrate apply` requires `--yes` unless `--dry-run` is used",
        ));
    }

    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let executable_plan = &built.runtime_plan.runtime.install_migrations;
    let manual_customer_migration_entries = manual_customer_migration_entries(&built.runtime_plan);

    if dry_run {
        return build_migrate_apply_report(
            &built.manifest,
            executable_plan,
            None,
            None,
            &manual_customer_migration_entries,
            true,
        );
    }

    let auth_package_report = run_auth_package_validate(&AuthPackageValidateInvocation {
        config_path: invocation.config_path.clone(),
    })?;
    if auth_package_report.status == ReportStatus::Unsafe {
        return Err(CliRunError::execution(format!(
            "auth package validation for `{}` is not green",
            built.manifest.id
        )));
    }

    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;
    let _runtime_guard = tokio_runtime.enter();
    let client = built
        .runtime_plan
        .runtime
        .data
        .connect_lazy_postgres()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to connect to the migration database for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let applied_keys = tokio_runtime
        .block_on(async { client.applied_migration_keys().await })
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to read applied migrations for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let pending_plan = pending_migration_plan(executable_plan, &applied_keys)?;
    let executed_statements = if pending_plan.ordered_steps().is_empty() {
        None
    } else {
        let mut registry = MigrationRegistry::new();
        registry.register(&pending_plan).map_err(|error| {
            CliRunError::execution(format!(
                "failed to register executable migrations for `{}`: {error}",
                built.manifest.id
            ))
        })?;
        let batch = built
            .runtime_plan
            .runtime
            .data
            .compile_migrations(&registry)
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to compile executable migrations for `{}`: {error}",
                    built.manifest.id
                ))
            })?;
        let execution = tokio_runtime
            .block_on(async { client.apply_migrations(&batch).await })
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to apply migrations for `{}`: {error}",
                    built.manifest.id
                ))
            })?;
        Some(execution.statements_executed)
    };

    build_migrate_apply_report(
        &built.manifest,
        executable_plan,
        Some(&applied_keys),
        executed_statements,
        &manual_customer_migration_entries,
        false,
    )
}

fn run_assets_publish(
    invocation: &AssetsPublishInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    if !dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`assets publish` requires `--yes` unless `--dry-run` is used",
        ));
    }

    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    if built.manifest.theme.asset_roots().is_empty() {
        let mut report = CommandReport::new(
            ["assets", "publish"],
            format!(
                "No theme asset roots are configured for customer app `{}`",
                built.manifest.id
            ),
        )
        .map_err(report_build_error)?
        .with_status(ReportStatus::Warning)
        .with_columns(["root", "status"])
        .map_err(report_build_error)?;
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "assets.roots.missing",
            format!(
                "customer app `{}` declares no theme asset roots, so asset publication is a no-op",
                built.manifest.id
            ),
        )?;
        return Ok(report);
    }

    let release_id = davenda_assets::ReleaseId::new(format!(
        "{}-{}-theme-assets",
        built.manifest.id, built.manifest.theme.active
    ))
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to allocate a theme asset release id for `{}`: {error}",
            built.manifest.id
        ))
    })?;
    let publication = built
        .manifest
        .theme
        .publication_plan(release_id, &built.app_root)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to prepare theme assets for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let cdn_base_url = built
        .runtime_plan
        .runtime
        .config
        .assets
        .cdn_base_url
        .as_deref()
        .ok_or_else(|| {
            CliRunError::execution(format!(
                "customer app `{}` cannot publish theme assets without `assets.cdn_base_url`",
                built.manifest.id
            ))
        })?;

    if dry_run {
        let manifest = publication
            .publish(&built.runtime_plan.runtime.storage_planner, cdn_base_url)
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to plan theme asset publication for `{}`: {error}",
                    built.manifest.id
                ))
            })?;
        return build_assets_publish_report(&built.manifest, &manifest, None, true);
    }

    let object_store = built
        .runtime_plan
        .runtime
        .object_store_client_config(&EnvironmentSecretResolver)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve storage backends for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let receipt = built
        .runtime_plan
        .runtime
        .storage_host_with_object_store(object_store)
        .publish_theme_assets(&publication)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to publish theme assets for `{}`: {error}",
                built.manifest.id
            ))
        })?;

    build_assets_publish_report(&built.manifest, receipt.manifest(), Some(&receipt), false)
}

fn run_cache_warm(
    invocation: &CacheWarmInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    warm_cache_routes(&built, &invocation.routes, &invocation.scope, dry_run)
}

fn run_cache_inspect(invocation: &CacheInspectInvocation) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    inspect_cache_route(&built, &invocation.route)
}

fn run_cache_invalidate(
    invocation: &CacheInvalidateInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    if !dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`cache invalidate` requires `--yes` unless `--dry-run` is used",
        ));
    }
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    invalidate_cache_tags(&built, &invocation.tags, dry_run)
}

fn run_jobs_status(invocation: &JobsStatusInvocation) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let queue_filter = invocation.queue.as_deref();
    let topology = built.runtime_plan.runtime.jobs.describe().clone();
    let mut report = CommandReport::new(
        ["jobs", "status"],
        format!("Jobs status for customer app `{}`", built.manifest.id),
    )
    .map_err(report_build_error)?
    .with_columns([
        "queue",
        "kind",
        "ready",
        "scheduled",
        "in_flight",
        "dead_letters",
        "registered_jobs",
    ])
    .map_err(report_build_error)?;

    let database_url = std::env::var("DATABASE_URL").ok();
    let jobs_host = if database_url.is_some() {
        Some(build_cli_jobs_host(
            &built,
            "platform-jobs-status",
            "status",
        )?)
    } else {
        None
    };
    let coordinator_state = jobs_host.as_ref().map(|(runtime, host)| {
        let _runtime_guard = runtime.enter();
        let coordinator = host.coordinator();
        (
            coordinator.ready_jobs().to_vec(),
            coordinator.scheduled_jobs().to_vec(),
            coordinator.in_flight_jobs().to_vec(),
            coordinator.dead_letters().to_vec(),
            host.registered_jobs.clone(),
            host.registered_event_subscriptions.clone(),
            host.coordinator().leadership().cloned(),
        )
    });

    for queue in &topology.queues {
        if queue_filter.is_some_and(|filter| filter != queue.name.as_str()) {
            continue;
        }
        let (ready, scheduled, in_flight, dead_letters, registered_jobs) = if let Some((
            ready_jobs,
            scheduled_jobs,
            in_flight_jobs,
            dead_letter_jobs,
            definitions,
            _,
            _,
        )) =
            coordinator_state.as_ref()
        {
            (
                ready_jobs
                    .iter()
                    .filter(|record| record.spec.queue == queue.name)
                    .count(),
                scheduled_jobs
                    .iter()
                    .filter(|record| record.spec.queue == queue.name)
                    .count(),
                in_flight_jobs
                    .iter()
                    .filter(|lease| lease.record.spec.queue == queue.name)
                    .count(),
                dead_letter_jobs
                    .iter()
                    .filter(|dead| dead.queue == queue.name)
                    .count(),
                definitions
                    .iter()
                    .filter(|definition| definition.queue == queue.name)
                    .count(),
            )
        } else {
            (0, 0, 0, 0, 0)
        };
        let row_status = if dead_letters > 0 {
            report = report.with_status(ReportStatus::Unsafe);
            "unsafe"
        } else if ready > 0 || scheduled > 0 || in_flight > 0 {
            if report.status == ReportStatus::Ok {
                report = report.with_status(ReportStatus::Warning);
            }
            "active"
        } else {
            "idle"
        };
        report.push_row(
            ReportRow::new()
                .with_cell("queue", queue.name.to_string())
                .map_err(report_build_error)?
                .with_cell("kind", queue.kind.to_string())
                .map_err(report_build_error)?
                .with_cell("ready", ready.to_string())
                .map_err(report_build_error)?
                .with_cell("scheduled", scheduled.to_string())
                .map_err(report_build_error)?
                .with_cell("in_flight", in_flight.to_string())
                .map_err(report_build_error)?
                .with_cell("dead_letters", dead_letters.to_string())
                .map_err(report_build_error)?
                .with_cell(
                    "registered_jobs",
                    format!("{registered_jobs} ({row_status})"),
                )
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "jobs.topology",
        format!(
            "backend={:?} work_queue={} scheduled_queue={} domain_events_queue={} dead_letter_queue={} default_retry_limit={}",
            built.runtime_plan.runtime.jobs.backend,
            topology.work_queue,
            topology.scheduled_queue,
            topology.domain_events_queue,
            topology.dead_letter_queue,
            built.runtime_plan.runtime.jobs.default_retry_limit
        ),
    )?;
    if let Some((runtime, host)) = jobs_host {
        let _runtime_guard = runtime.enter();
        let leadership = host.coordinator().leadership().cloned();
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "jobs.runtime",
            format!(
                "registered_jobs={} registered_event_subscriptions={} leadership={}",
                host.registered_jobs.len(),
                host.registered_event_subscriptions.len(),
                leadership
                    .map(|value| format!("{} until {}", value.node_id, value.lease_until))
                    .unwrap_or_else(|| "none".to_string())
            ),
        )?;
    } else {
        report = report.with_status(ReportStatus::Warning);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "jobs.runtime.unavailable",
            format!(
                "live jobs coordinator state is unavailable for `{}`: set DATABASE_URL to inspect distributed queue health; showing queue topology only",
                built.manifest.id
            ),
        )?;
    }

    Ok(report)
}

fn run_jobs_ready(invocation: &JobsReadyInvocation) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let topology = built.runtime_plan.runtime.jobs.describe().clone();
    let queue_filter = invocation.queue.as_deref();
    if let Some(filter) = queue_filter {
        let known_queues = topology
            .queues
            .iter()
            .map(|queue| queue.name.to_string())
            .collect::<Vec<_>>();
        if !known_queues.iter().any(|queue| queue == filter) {
            return Err(CliRunError::execution(format!(
                "queue filter `{filter}` is not defined for customer app `{}`; expected one of: {}",
                built.manifest.id,
                known_queues.join(", ")
            )));
        }
    }

    let database_url = std::env::var("DATABASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            CliRunError::execution(format!(
                "live jobs coordinator state is required to inspect ready jobs for `{}`: set DATABASE_URL before running `jobs ready`",
                built.manifest.id
            ))
        })?;
    let _database_url = database_url;

    let (runtime, host) = build_cli_jobs_host(&built, "platform-jobs-ready", "ready")?;
    let _runtime_guard = runtime.enter();

    build_jobs_ready_report(
        &built.manifest.id.to_string(),
        &topology,
        built.runtime_plan.runtime.jobs.backend,
        built.runtime_plan.runtime.jobs.default_retry_limit,
        host.coordinator().ready_jobs(),
        queue_filter,
        invocation.limit,
    )
}

fn build_jobs_ready_report(
    app_id: &str,
    topology: &davenda_jobs::QueueTopology,
    backend: davenda_config::JobBackend,
    default_retry_limit: u32,
    ready_jobs: &[davenda_jobs::QueuedJobRecord],
    queue_filter: Option<&str>,
    limit: usize,
) -> Result<CommandReport, CliRunError> {
    let mut report = CommandReport::new(
        ["jobs", "ready"],
        format!("Ready jobs for customer app `{app_id}`"),
    )
    .map_err(report_build_error)?
    .with_columns([
        "job_id",
        "job_name",
        "queue",
        "attempts",
        "enqueued_at",
        "idempotency_key",
        "payload",
    ])
    .map_err(report_build_error)?;

    let mut ready_jobs = ready_jobs
        .iter()
        .filter(|record| queue_filter.map_or(true, |filter| record.spec.queue.as_str() == filter))
        .collect::<Vec<_>>();
    ready_jobs.sort_by(|left, right| {
        left.enqueued_at
            .cmp(&right.enqueued_at)
            .then_with(|| left.spec.job_id.as_str().cmp(right.spec.job_id.as_str()))
    });

    let total_count = ready_jobs.len();
    let limited = ready_jobs.into_iter().take(limit).collect::<Vec<_>>();

    for record in &limited {
        report.push_row(
            ReportRow::new()
                .with_cell("job_id", record.spec.job_id.to_string())
                .map_err(report_build_error)?
                .with_cell("job_name", record.spec.job_name.to_string())
                .map_err(report_build_error)?
                .with_cell("queue", record.spec.queue.to_string())
                .map_err(report_build_error)?
                .with_cell("attempts", record.attempts.to_string())
                .map_err(report_build_error)?
                .with_cell(
                    "enqueued_at",
                    record.enqueued_at.as_unix_seconds().to_string(),
                )
                .map_err(report_build_error)?
                .with_cell(
                    "idempotency_key",
                    record
                        .spec
                        .idempotency_key
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "none".to_string()),
                )
                .map_err(report_build_error)?
                .with_cell("payload", record.spec.payload_description.clone())
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "jobs.ready",
        format!(
            "queue_filter={} limit={} returned {} of {} ready job(s)",
            queue_filter.unwrap_or("all"),
            limit,
            limited.len(),
            total_count
        ),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "jobs.topology",
        format!(
            "backend={:?} work_queue={} scheduled_queue={} domain_events_queue={} dead_letter_queue={} default_retry_limit={}",
            backend,
            topology.work_queue,
            topology.scheduled_queue,
            topology.domain_events_queue,
            topology.dead_letter_queue,
            default_retry_limit
        ),
    )?;

    Ok(report)
}

fn run_jobs_run(
    invocation: &JobsRunInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let topology = built.runtime_plan.runtime.jobs.describe().clone();
    let queue_filter = invocation.queue.as_deref();
    let executable_queues =
        executable_jobs_queues(&built.manifest.id.to_string(), &topology, queue_filter)?;
    let worker_id = invocation
        .worker_id
        .clone()
        .unwrap_or_else(|| format!("platform-jobs-run:{}", built.manifest.id));
    let mut report = CommandReport::new(
        ["jobs", "run"],
        if dry_run {
            format!(
                "Planned worker batch for customer app `{}`",
                built.manifest.id
            )
        } else {
            format!(
                "Executed worker batch for customer app `{}`",
                built.manifest.id
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns([
        "job_id",
        "job_name",
        "queue",
        "worker_id",
        "attempt",
        "status",
        "detail",
    ])
    .map_err(report_build_error)?;

    let probe_runtime = build_cli_async_runtime()?;
    if let Some(reason) =
        live_jobs_state_unavailable_reason(&built, &probe_runtime, "run queued jobs")?
    {
        report = report.with_status(ReportStatus::Warning);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "jobs.runtime.unavailable",
            reason,
        )?;
        return Ok(report);
    }

    let (runtime, mut host) = build_cli_jobs_host(&built, "platform-jobs-run", "run")?;
    let _runtime_guard = runtime.enter();
    let now_unix_seconds = unix_timestamp_now()?;
    let now = JobInstant::from_unix_seconds(now_unix_seconds);

    let due_scheduled_jobs = host
        .coordinator()
        .scheduled_jobs()
        .iter()
        .filter_map(|record| {
            let scheduled_for = record.spec.scheduled_for?;
            if scheduled_for.as_unix_seconds() > now_unix_seconds {
                return None;
            }
            queue_filter
                .map_or(record.spec.queue == topology.scheduled_queue, |filter| {
                    record.spec.queue.as_str() == filter
                })
                .then_some((
                    record.spec.job_id.to_string(),
                    record.spec.job_name.to_string(),
                    record.spec.queue.to_string(),
                    scheduled_for.as_unix_seconds(),
                ))
        })
        .collect::<Vec<_>>();

    if dry_run {
        for (job_id, job_name, queue, scheduled_for) in
            due_scheduled_jobs.iter().take(invocation.limit)
        {
            report.push_row(
                ReportRow::new()
                    .with_cell("job_id", job_id.clone())
                    .map_err(report_build_error)?
                    .with_cell("job_name", job_name.clone())
                    .map_err(report_build_error)?
                    .with_cell("queue", queue.clone())
                    .map_err(report_build_error)?
                    .with_cell("worker_id", worker_id.clone())
                    .map_err(report_build_error)?
                    .with_cell("attempt", "next".to_string())
                    .map_err(report_build_error)?
                    .with_cell("status", "planned")
                    .map_err(report_build_error)?
                    .with_cell(
                        "detail",
                        format!(
                            "due scheduled job would be promoted and executed at {scheduled_for}"
                        ),
                    )
                    .map_err(report_build_error)?,
            );
        }

        let remaining = invocation.limit.saturating_sub(report.rows.len());
        for record in host
            .coordinator()
            .ready_jobs()
            .iter()
            .filter(|record| {
                executable_queues
                    .iter()
                    .any(|queue| queue == &record.spec.queue)
            })
            .take(remaining)
        {
            report.push_row(
                ReportRow::new()
                    .with_cell("job_id", record.spec.job_id.to_string())
                    .map_err(report_build_error)?
                    .with_cell("job_name", record.spec.job_name.to_string())
                    .map_err(report_build_error)?
                    .with_cell("queue", record.spec.queue.to_string())
                    .map_err(report_build_error)?
                    .with_cell("worker_id", worker_id.clone())
                    .map_err(report_build_error)?
                    .with_cell("attempt", (record.attempts.saturating_add(1)).to_string())
                    .map_err(report_build_error)?
                    .with_cell("status", "planned")
                    .map_err(report_build_error)?
                    .with_cell(
                        "detail",
                        "ready job would be leased and executed in this worker batch",
                    )
                    .map_err(report_build_error)?,
            );
        }

        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "jobs.run.plan",
            format!(
                "worker_id={} queue_filter={} limit={} due_scheduled={} ready_jobs={}",
                worker_id,
                queue_filter.unwrap_or("all"),
                invocation.limit,
                due_scheduled_jobs.len(),
                host.coordinator()
                    .ready_jobs()
                    .iter()
                    .filter(|record| {
                        executable_queues
                            .iter()
                            .any(|queue| queue == &record.spec.queue)
                    })
                    .count()
            ),
        )?;
        push_jobs_topology_diagnostic(
            &mut report,
            &topology,
            built.runtime_plan.runtime.jobs.backend,
            built.runtime_plan.runtime.jobs.default_retry_limit,
        )?;
        return Ok(report);
    }

    let wasm_host = built
        .runtime_plan
        .runtime
        .wasm_host_with_secret_resolver(&EnvironmentSecretResolver)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build worker execution host for `{}`: {error}",
                built.manifest.id
            ))
        })?;

    let mut scheduler_warning = None;
    if executable_queues
        .iter()
        .any(|queue| queue == &topology.scheduled_queue)
        && !due_scheduled_jobs.is_empty()
    {
        match host
            .acquire_scheduler_leadership(now, Duration::from_secs(30))
            .and_then(|_| host.promote_due_jobs(now))
        {
            Ok(promoted) => {
                push_report_diagnostic(
                    &mut report,
                    DiagnosticSeverity::Info,
                    "jobs.run.promote",
                    format!(
                        "promoted {} due scheduled job(s) before worker leasing",
                        promoted.len()
                    ),
                )?;
            }
            Err(error) => {
                report = report.with_status(ReportStatus::Warning);
                let message =
                    format!("failed to promote due scheduled jobs before execution: {error}");
                scheduler_warning = Some(message.clone());
                push_report_diagnostic(
                    &mut report,
                    DiagnosticSeverity::Warning,
                    "jobs.run.promote.unavailable",
                    message,
                )?;
            }
        }
    }

    let mut completed_jobs = 0usize;
    let mut retried_jobs = 0usize;
    let mut dead_lettered_jobs = 0usize;
    let mut remaining = invocation.limit;

    for queue in executable_queues {
        if remaining == 0 {
            break;
        }

        let leases = host
            .lease_ready_jobs(
                &queue,
                worker_id.clone(),
                JobInstant::from_unix_seconds(unix_timestamp_now()?),
                Duration::from_secs(60),
                remaining,
            )
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to lease ready jobs from `{queue}` for `{}`: {error}",
                    built.manifest.id
                ))
            })?;

        for lease in leases {
            let attempt = lease.record.attempts.saturating_add(1);
            match wasm_host.execute_leased_job(&lease) {
                Ok(Some(receipt)) => {
                    host.acknowledge_completed(
                        &lease,
                        JobInstant::from_unix_seconds(unix_timestamp_now()?),
                    )
                    .map_err(|error| {
                        CliRunError::execution(format!(
                            "failed to acknowledge completed job `{}` for `{}`: {error}",
                            lease.record.spec.job_id, built.manifest.id
                        ))
                    })?;
                    completed_jobs += 1;
                    report.push_row(
                        ReportRow::new()
                            .with_cell("job_id", lease.record.spec.job_id.to_string())
                            .map_err(report_build_error)?
                            .with_cell("job_name", lease.record.spec.job_name.to_string())
                            .map_err(report_build_error)?
                            .with_cell("queue", lease.record.spec.queue.to_string())
                            .map_err(report_build_error)?
                            .with_cell("worker_id", worker_id.clone())
                            .map_err(report_build_error)?
                            .with_cell("attempt", attempt.to_string())
                            .map_err(report_build_error)?
                            .with_cell("status", "completed")
                            .map_err(report_build_error)?
                            .with_cell(
                                "detail",
                                format!(
                                    "extension={} outcome={:?} runtime_ms={}",
                                    receipt.extension_id,
                                    receipt.outcome,
                                    receipt.runtime.as_millis()
                                ),
                            )
                            .map_err(report_build_error)?,
                    );
                }
                Ok(None) => {
                    let detail = format!(
                        "job `{}` has no installed handler capable of execution",
                        lease.record.spec.job_name
                    );
                    let disposition = host
                        .acknowledge_failed(
                            &lease,
                            JobInstant::from_unix_seconds(unix_timestamp_now()?),
                            DeadLetterReason::PolicyViolation,
                            detail.clone(),
                        )
                        .map_err(|error| {
                            CliRunError::execution(format!(
                                "failed to acknowledge non-executable job `{}` for `{}`: {error}",
                                lease.record.spec.job_id, built.manifest.id
                            ))
                        })?;
                    push_jobs_run_failure_row(
                        &mut report,
                        &lease,
                        &worker_id,
                        attempt,
                        disposition,
                        detail,
                        &mut retried_jobs,
                        &mut dead_lettered_jobs,
                    )?;
                }
                Err(error) => {
                    let detail = error.to_string();
                    let disposition = host
                        .acknowledge_failed(
                            &lease,
                            JobInstant::from_unix_seconds(unix_timestamp_now()?),
                            jobs_run_dead_letter_reason(&error),
                            detail.clone(),
                        )
                        .map_err(|ack_error| {
                            CliRunError::execution(format!(
                                "failed to acknowledge failed job `{}` for `{}`: {ack_error}",
                                lease.record.spec.job_id, built.manifest.id
                            ))
                        })?;
                    push_jobs_run_failure_row(
                        &mut report,
                        &lease,
                        &worker_id,
                        attempt,
                        disposition,
                        detail,
                        &mut retried_jobs,
                        &mut dead_lettered_jobs,
                    )?;
                }
            }
            remaining = remaining.saturating_sub(1);
            if remaining == 0 {
                break;
            }
        }
    }

    if dead_lettered_jobs > 0 {
        report = report.with_status(ReportStatus::Unsafe);
    } else if retried_jobs > 0 || scheduler_warning.is_some() {
        report = report.with_status(ReportStatus::Warning);
    }

    push_report_diagnostic(
        &mut report,
        if dead_lettered_jobs > 0 {
            DiagnosticSeverity::Warning
        } else {
            DiagnosticSeverity::Info
        },
        "jobs.run.result",
        format!(
            "worker_id={} queue_filter={} limit={} completed={} retried={} dead_lettered={}",
            worker_id,
            queue_filter.unwrap_or("all"),
            invocation.limit,
            completed_jobs,
            retried_jobs,
            dead_lettered_jobs
        ),
    )?;
    push_jobs_topology_diagnostic(
        &mut report,
        &topology,
        built.runtime_plan.runtime.jobs.backend,
        built.runtime_plan.runtime.jobs.default_retry_limit,
    )?;

    Ok(report)
}

fn executable_jobs_queues(
    app_id: &str,
    topology: &davenda_jobs::QueueTopology,
    queue_filter: Option<&str>,
) -> Result<Vec<davenda_jobs::JobQueueName>, CliRunError> {
    if let Some(filter) = queue_filter {
        let known_queues = topology
            .queues
            .iter()
            .map(|queue| queue.name.to_string())
            .collect::<Vec<_>>();
        if !known_queues.iter().any(|queue| queue == filter) {
            return Err(CliRunError::execution(format!(
                "queue filter `{filter}` is not defined for customer app `{app_id}`; expected one of: {}",
                known_queues.join(", ")
            )));
        }
        if filter == topology.dead_letter_queue.as_str() {
            return Err(CliRunError::execution(format!(
                "`jobs run` cannot execute the dead-letter queue for customer app `{app_id}`; use `jobs retry` instead"
            )));
        }
    }

    Ok(topology
        .queues
        .iter()
        .filter(|queue| queue.kind != QueueKind::DeadLetter)
        .filter(|queue| queue_filter.map_or(true, |filter| queue.name.as_str() == filter))
        .map(|queue| queue.name.clone())
        .collect())
}

fn push_jobs_topology_diagnostic(
    report: &mut CommandReport,
    topology: &davenda_jobs::QueueTopology,
    backend: davenda_config::JobBackend,
    default_retry_limit: u32,
) -> Result<(), CliRunError> {
    push_report_diagnostic(
        report,
        DiagnosticSeverity::Info,
        "jobs.topology",
        format!(
            "backend={:?} work_queue={} scheduled_queue={} domain_events_queue={} dead_letter_queue={} default_retry_limit={}",
            backend,
            topology.work_queue,
            topology.scheduled_queue,
            topology.domain_events_queue,
            topology.dead_letter_queue,
            default_retry_limit
        ),
    )
}

fn jobs_run_dead_letter_reason(
    _error: &davenda_runtime::LiveWasmExecutionError,
) -> DeadLetterReason {
    DeadLetterReason::HandlerPanic
}

fn push_jobs_run_failure_row(
    report: &mut CommandReport,
    lease: &davenda_jobs::JobLease,
    worker_id: &str,
    attempt: u32,
    disposition: JobFailureDisposition,
    detail: String,
    retried_jobs: &mut usize,
    dead_lettered_jobs: &mut usize,
) -> Result<(), CliRunError> {
    let (status, detail) = match disposition {
        JobFailureDisposition::Retried {
            next_attempt_at,
            queue,
            ..
        } => {
            *retried_jobs += 1;
            (
                "retried",
                format!(
                    "{detail}; requeued to {queue} for retry at {}",
                    next_attempt_at.as_unix_seconds()
                ),
            )
        }
        JobFailureDisposition::DeadLettered(outcome) => {
            *dead_lettered_jobs += 1;
            (
                "dead_lettered",
                format!(
                    "{detail}; dead-letter={} reason={:?}",
                    outcome.dead_letter_id, outcome.reason
                ),
            )
        }
    };

    report.push_row(
        ReportRow::new()
            .with_cell("job_id", lease.record.spec.job_id.to_string())
            .map_err(report_build_error)?
            .with_cell("job_name", lease.record.spec.job_name.to_string())
            .map_err(report_build_error)?
            .with_cell("queue", lease.record.spec.queue.to_string())
            .map_err(report_build_error)?
            .with_cell("worker_id", worker_id.to_string())
            .map_err(report_build_error)?
            .with_cell("attempt", attempt.to_string())
            .map_err(report_build_error)?
            .with_cell("status", status)
            .map_err(report_build_error)?
            .with_cell("detail", detail)
            .map_err(report_build_error)?,
    );
    Ok(())
}

fn run_jobs_dead_letters(
    invocation: &JobsDeadLettersInvocation,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let queue_filter = invocation.queue.as_deref();
    let mut report = CommandReport::new(
        ["jobs", "dead-letters"],
        format!("Dead-letter jobs for customer app `{}`", built.manifest.id),
    )
    .map_err(report_build_error)?
    .with_columns([
        "dead_letter_id",
        "job_id",
        "queue",
        "reason",
        "failed_attempts",
        "routed_to",
        "error_message",
    ])
    .map_err(report_build_error)?;

    let database_url = std::env::var("DATABASE_URL").ok();
    let jobs_host = if database_url.is_some() {
        Some(build_cli_jobs_host(
            &built,
            "platform-jobs-dead-letters",
            "dead-letter",
        )?)
    } else {
        None
    };

    if let Some((runtime, host)) = jobs_host {
        let _runtime_guard = runtime.enter();
        let mut dead_letters = host
            .coordinator()
            .dead_letters()
            .iter()
            .filter(|dead_letter| {
                queue_filter.map_or(true, |filter| dead_letter.queue.as_str() == filter)
            })
            .collect::<Vec<_>>();
        dead_letters.sort_by(|left, right| {
            left.dead_letter_id
                .as_str()
                .cmp(right.dead_letter_id.as_str())
        });

        let limited = dead_letters
            .into_iter()
            .take(invocation.limit)
            .collect::<Vec<_>>();
        if !limited.is_empty() {
            report = report.with_status(ReportStatus::Unsafe);
        }

        for dead_letter in &limited {
            report.push_row(
                ReportRow::new()
                    .with_cell("dead_letter_id", dead_letter.dead_letter_id.to_string())
                    .map_err(report_build_error)?
                    .with_cell("job_id", dead_letter.job_id.to_string())
                    .map_err(report_build_error)?
                    .with_cell("queue", dead_letter.queue.to_string())
                    .map_err(report_build_error)?
                    .with_cell("reason", format!("{:?}", dead_letter.reason))
                    .map_err(report_build_error)?
                    .with_cell("failed_attempts", dead_letter.failed_attempts.to_string())
                    .map_err(report_build_error)?
                    .with_cell(
                        "routed_to",
                        dead_letter
                            .routed_to
                            .as_ref()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "none".to_string()),
                    )
                    .map_err(report_build_error)?
                    .with_cell("error_message", dead_letter.error_message.clone())
                    .map_err(report_build_error)?,
            );
        }

        push_report_diagnostic(
            &mut report,
            if limited.is_empty() {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Warning
            },
            "jobs.dead_letters",
            format!(
                "queue_filter={} limit={} returned {} dead-letter record(s)",
                queue_filter.unwrap_or("all"),
                invocation.limit,
                limited.len()
            ),
        )?;
    } else {
        report = report.with_status(ReportStatus::Warning);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "jobs.runtime.unavailable",
            format!(
                "live jobs coordinator state is unavailable for `{}`: set DATABASE_URL to inspect dead-letter state",
                built.manifest.id
            ),
        )?;
    }

    let topology = built.runtime_plan.runtime.jobs.describe().clone();
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "jobs.topology",
        format!(
            "backend={:?} work_queue={} scheduled_queue={} domain_events_queue={} dead_letter_queue={} default_retry_limit={}",
            built.runtime_plan.runtime.jobs.backend,
            topology.work_queue,
            topology.scheduled_queue,
            topology.domain_events_queue,
            topology.dead_letter_queue,
            built.runtime_plan.runtime.jobs.default_retry_limit
        ),
    )?;

    Ok(report)
}

fn run_jobs_in_flight(invocation: &JobsInFlightInvocation) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let queue_filter = invocation.queue.as_deref();
    let worker_filter = invocation.worker_id.as_deref();
    let mut report = CommandReport::new(
        ["jobs", "in-flight"],
        format!("In-flight jobs for customer app `{}`", built.manifest.id),
    )
    .map_err(report_build_error)?
    .with_columns([
        "job_id",
        "queue",
        "worker_id",
        "attempts",
        "leased_at",
        "lease_until",
        "status",
    ])
    .map_err(report_build_error)?;

    let probe_runtime = build_cli_async_runtime()?;
    if let Some(reason) =
        live_jobs_state_unavailable_reason(&built, &probe_runtime, "inspect in-flight job state")?
    {
        report = report.with_status(ReportStatus::Warning);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "jobs.runtime.unavailable",
            reason,
        )?;
    } else {
        let (runtime, host) = build_cli_jobs_host(&built, "platform-jobs-in-flight", "in-flight")?;
        let _runtime_guard = runtime.enter();
        let now_unix_seconds = unix_timestamp_now()?;
        let mut leases = host
            .coordinator()
            .in_flight_jobs()
            .iter()
            .filter(|lease| {
                queue_filter.map_or(true, |filter| lease.record.spec.queue.as_str() == filter)
                    && worker_filter.map_or(true, |filter| lease.worker_id == filter)
            })
            .collect::<Vec<_>>();
        leases.sort_by(|left, right| {
            left.lease_until.cmp(&right.lease_until).then_with(|| {
                left.record
                    .spec
                    .job_id
                    .as_str()
                    .cmp(right.record.spec.job_id.as_str())
            })
        });

        let total_count = leases.len();
        let limited = leases
            .into_iter()
            .take(invocation.limit)
            .collect::<Vec<_>>();
        let expired_count = limited
            .iter()
            .filter(|lease| lease.lease_until.as_unix_seconds() <= now_unix_seconds)
            .count();
        if expired_count > 0 {
            report = report.with_status(ReportStatus::Unsafe);
        }

        for lease in &limited {
            let expired = lease.lease_until.as_unix_seconds() <= now_unix_seconds;
            report.push_row(
                ReportRow::new()
                    .with_cell("job_id", lease.record.spec.job_id.to_string())
                    .map_err(report_build_error)?
                    .with_cell("queue", lease.record.spec.queue.to_string())
                    .map_err(report_build_error)?
                    .with_cell("worker_id", lease.worker_id.clone())
                    .map_err(report_build_error)?
                    .with_cell("attempts", lease.record.attempts.to_string())
                    .map_err(report_build_error)?
                    .with_cell("leased_at", lease.leased_at.as_unix_seconds().to_string())
                    .map_err(report_build_error)?
                    .with_cell(
                        "lease_until",
                        lease.lease_until.as_unix_seconds().to_string(),
                    )
                    .map_err(report_build_error)?
                    .with_cell("status", if expired { "expired" } else { "leased" })
                    .map_err(report_build_error)?,
            );
        }

        push_report_diagnostic(
            &mut report,
            if expired_count > 0 {
                DiagnosticSeverity::Warning
            } else {
                DiagnosticSeverity::Info
            },
            "jobs.in_flight",
            format!(
                "queue_filter={} worker_filter={} limit={} returned {} of {} in-flight job(s)",
                queue_filter.unwrap_or("all"),
                worker_filter.unwrap_or("all"),
                invocation.limit,
                limited.len(),
                total_count
            ),
        )?;
        if expired_count > 0 {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Warning,
                "jobs.in_flight.expired",
                format!("{expired_count} leased job(s) have expired worker leases"),
            )?;
        }
    }

    let topology = built.runtime_plan.runtime.jobs.describe().clone();
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "jobs.topology",
        format!(
            "backend={:?} work_queue={} scheduled_queue={} domain_events_queue={} dead_letter_queue={} default_retry_limit={}",
            built.runtime_plan.runtime.jobs.backend,
            topology.work_queue,
            topology.scheduled_queue,
            topology.domain_events_queue,
            topology.dead_letter_queue,
            built.runtime_plan.runtime.jobs.default_retry_limit
        ),
    )?;

    Ok(report)
}

fn run_jobs_retry(
    invocation: &JobsRetryInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    if !dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`jobs retry` requires `--yes` unless `--dry-run` is used",
        ));
    }

    let mut report = CommandReport::new(
        ["jobs", "retry"],
        if dry_run {
            format!(
                "Planned retry of dead-letter `{}` for customer app `{}`",
                invocation.dead_letter_id, built.manifest.id
            )
        } else {
            format!(
                "Retried dead-letter `{}` for customer app `{}`",
                invocation.dead_letter_id, built.manifest.id
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns(["dead_letter_id", "job_id", "queue", "status", "detail"])
    .map_err(report_build_error)?;

    let probe_runtime = build_cli_async_runtime()?;
    if let Some(reason) =
        live_jobs_state_unavailable_reason(&built, &probe_runtime, "retry dead-lettered jobs")?
    {
        report = report.with_status(ReportStatus::Warning);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "jobs.runtime.unavailable",
            reason,
        )?;
        return Ok(report);
    }

    let (runtime, mut host) = build_cli_jobs_host(&built, "platform-jobs-retry", "retry")?;
    let _runtime_guard = runtime.enter();
    let dead_letter = host
        .coordinator()
        .dead_letters()
        .iter()
        .find(|outcome| outcome.dead_letter_id.as_str() == invocation.dead_letter_id)
        .cloned()
        .ok_or_else(|| {
            CliRunError::execution(format!(
                "dead-letter `{}` does not exist for customer app `{}`",
                invocation.dead_letter_id, built.manifest.id
            ))
        })?;

    let planned_job_id = dead_letter.job_id.to_string();
    let planned_queue = dead_letter.queue.to_string();
    if dry_run {
        report.push_row(
            ReportRow::new()
                .with_cell("dead_letter_id", dead_letter.dead_letter_id.to_string())
                .map_err(report_build_error)?
                .with_cell("job_id", planned_job_id)
                .map_err(report_build_error)?
                .with_cell("queue", planned_queue)
                .map_err(report_build_error)?
                .with_cell("status", "planned")
                .map_err(report_build_error)?
                .with_cell(
                    "detail",
                    "dead-letter record will be requeued onto the ready work set",
                )
                .map_err(report_build_error)?,
        );
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "jobs.retry.plan",
            format!(
                "dead-letter `{}` would be retried immediately from queue `{}`",
                dead_letter.dead_letter_id, dead_letter.queue
            ),
        )?;
        return Ok(report);
    }

    let retried_job_id = host
        .retry_dead_letter(invocation.dead_letter_id.clone(), unix_timestamp_now()?)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to retry dead-letter `{}` for `{}`: {error}",
                invocation.dead_letter_id, built.manifest.id
            ))
        })?;
    report.push_row(
        ReportRow::new()
            .with_cell("dead_letter_id", dead_letter.dead_letter_id.to_string())
            .map_err(report_build_error)?
            .with_cell("job_id", retried_job_id.to_string())
            .map_err(report_build_error)?
            .with_cell("queue", planned_queue)
            .map_err(report_build_error)?
            .with_cell("status", "retried")
            .map_err(report_build_error)?
            .with_cell(
                "detail",
                "dead-letter record was removed and requeued for immediate execution",
            )
            .map_err(report_build_error)?,
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "jobs.retry.result",
        format!(
            "retried dead-letter `{}` as job `{}`",
            dead_letter.dead_letter_id, retried_job_id
        ),
    )?;

    Ok(report)
}

fn run_jobs_promote(
    invocation: &JobsPromoteInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    if !dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`jobs promote` requires `--yes` unless `--dry-run` is used",
        ));
    }

    let now_unix_seconds = unix_timestamp_now()?;
    let now = JobInstant::from_unix_seconds(now_unix_seconds);
    let mut report = CommandReport::new(
        ["jobs", "promote"],
        if dry_run {
            format!(
                "Planned promotion of due scheduled jobs for customer app `{}`",
                built.manifest.id
            )
        } else {
            format!(
                "Promoted due scheduled jobs for customer app `{}`",
                built.manifest.id
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns(["job_id", "queue", "scheduled_for", "status"])
    .map_err(report_build_error)?;

    let probe_runtime = build_cli_async_runtime()?;
    if let Some(reason) =
        live_jobs_state_unavailable_reason(&built, &probe_runtime, "promote due scheduled jobs")?
    {
        report = report.with_status(ReportStatus::Warning);
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "jobs.runtime.unavailable",
            reason,
        )?;
        return Ok(report);
    }

    let (runtime, mut host) = build_cli_jobs_host(&built, "platform-jobs-promote", "promote")?;
    let _runtime_guard = runtime.enter();
    let due_jobs = host
        .coordinator()
        .scheduled_jobs()
        .iter()
        .filter_map(|record| {
            let scheduled_for = record.spec.scheduled_for?;
            (scheduled_for.as_unix_seconds() <= now_unix_seconds).then_some((
                record.spec.job_id.to_string(),
                record.spec.queue.to_string(),
                scheduled_for.as_unix_seconds(),
            ))
        })
        .collect::<Vec<_>>();

    if dry_run {
        for (job_id, queue, scheduled_for) in &due_jobs {
            report.push_row(
                ReportRow::new()
                    .with_cell("job_id", job_id.clone())
                    .map_err(report_build_error)?
                    .with_cell("queue", queue.clone())
                    .map_err(report_build_error)?
                    .with_cell("scheduled_for", scheduled_for.to_string())
                    .map_err(report_build_error)?
                    .with_cell("status", "planned")
                    .map_err(report_build_error)?,
            );
        }
        if due_jobs.is_empty() {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Info,
                "jobs.promote.plan",
                "no due scheduled jobs are ready for promotion".to_string(),
            )?;
        } else {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Info,
                "jobs.promote.plan",
                format!("{} due scheduled job(s) would be promoted", due_jobs.len()),
            )?;
        }
        return Ok(report);
    }

    host.acquire_scheduler_leadership(now, Duration::from_secs(30))
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to acquire scheduler leadership for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let promoted = host.promote_due_jobs(now).map_err(|error| {
        CliRunError::execution(format!(
            "failed to promote due jobs for `{}`: {error}",
            built.manifest.id
        ))
    })?;
    for job_id in &promoted {
        let (queue, scheduled_for) = due_jobs
            .iter()
            .find(|(candidate, _, _)| candidate == job_id.as_str())
            .map(|(_, queue, scheduled_for)| (queue.clone(), scheduled_for.to_string()))
            .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
        report.push_row(
            ReportRow::new()
                .with_cell("job_id", job_id.to_string())
                .map_err(report_build_error)?
                .with_cell("queue", queue)
                .map_err(report_build_error)?
                .with_cell("scheduled_for", scheduled_for)
                .map_err(report_build_error)?
                .with_cell("status", "promoted")
                .map_err(report_build_error)?,
        );
    }
    if promoted.is_empty() {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "jobs.promote.result",
            "no due scheduled jobs were promoted".to_string(),
        )?;
    } else {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "jobs.promote.result",
            format!(
                "promoted {} scheduled job(s) into the ready queue",
                promoted.len()
            ),
        )?;
    }

    Ok(report)
}

fn run_tls_status(config_path: &Path) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(config_path, true)?;
    if built.runtime_plan.runtime.tls.mode == davenda_config::TlsMode::External {
        let mut report = CommandReport::new(
            ["tls", "status"],
            format!("TLS status for customer app `{}`", built.manifest.id),
        )
        .map_err(report_build_error)?
        .with_columns([
            "certificate",
            "status",
            "provider",
            "hostnames",
            "not_after",
            "replacement",
        ])
        .map_err(report_build_error)?;
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "tls.mode",
            format!(
                "mode={:?} edge_mode={:?} provider=none inventory=0 queued_renewals=0 pending_challenges=0 hot_reload_events=0",
                built.runtime_plan.runtime.tls.mode, built.runtime_plan.runtime.tls.edge_mode
            ),
        )?;
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "tls.external_termination",
            "TLS is externally terminated for this customer app, so the platform does not manage certificate inventory",
        )?;
        return Ok(report);
    }
    let host = built
        .runtime_plan
        .runtime
        .tls_validation_host_with_secret_resolver(&EnvironmentSecretResolver)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build TLS host for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let snapshot = host.status();

    let mut report = CommandReport::new(
        ["tls", "status"],
        format!("TLS status for customer app `{}`", built.manifest.id),
    )
    .map_err(report_build_error)?
    .with_columns([
        "certificate",
        "status",
        "provider",
        "hostnames",
        "not_after",
        "replacement",
    ])
    .map_err(report_build_error)?;

    for record in snapshot.inventory.certificates() {
        report.push_row(
            ReportRow::new()
                .with_cell("certificate", record.id.to_string())
                .map_err(report_build_error)?
                .with_cell("status", format!("{:?}", record.status).to_lowercase())
                .map_err(report_build_error)?
                .with_cell("provider", format!("{:?}", record.provider).to_lowercase())
                .map_err(report_build_error)?
                .with_cell(
                    "hostnames",
                    record
                        .bindings
                        .iter()
                        .map(|binding| binding.hostname.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .map_err(report_build_error)?
                .with_cell("not_after", record.not_after.to_string())
                .map_err(report_build_error)?
                .with_cell(
                    "replacement",
                    record
                        .replacing_certificate
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "none".to_string()),
                )
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "tls.mode",
        format!(
            "mode={:?} edge_mode={:?} provider={} inventory={} queued_renewals={} pending_challenges={} hot_reload_events={}",
            snapshot.mode,
            snapshot.edge_mode,
            snapshot
                .provider
                .map(|provider| provider.to_string())
                .unwrap_or_else(|| "none".to_string()),
            snapshot.inventory.certificates().len(),
            snapshot.queued_renewals.len(),
            snapshot.pending_challenges.len(),
            snapshot.hot_reload_events.len()
        ),
    )?;
    if snapshot.inventory.certificates().is_empty() {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "tls.inventory.empty",
            "no managed TLS certificates are currently present in the platform control plane",
        )?;
    }

    Ok(report)
}

fn run_tls_validate_challenge(
    invocation: &TlsValidateChallengeInvocation,
) -> Result<CommandReport, CliRunError> {
    run_tls_validate_challenge_impl(invocation)
}

fn customer_app_tls_bindings(
    built: &BuiltCustomerAppContext,
) -> Result<Vec<HostnameBinding>, CliRunError> {
    if built.manifest.domains.is_empty() {
        return Err(CliRunError::execution(format!(
            "customer app `{}` does not declare any TLS hostnames to validate",
            built.manifest.id
        )));
    }

    let customer_app_id = CustomerAppId::new(built.manifest.id.to_string()).map_err(|error| {
        CliRunError::execution(format!(
            "customer app `{}` has an invalid TLS customer-app id: {error}",
            built.manifest.id
        ))
    })?;

    built
        .manifest
        .domains
        .iter()
        .map(|domain| {
            let hostname = Hostname::new(domain.hostname.clone()).map_err(|error| {
                CliRunError::execution(format!(
                    "customer app `{}` has an invalid TLS hostname `{}`: {error}",
                    built.manifest.id, domain.hostname
                ))
            })?;
            Ok(HostnameBinding::new(hostname, customer_app_id.clone()))
        })
        .collect()
}

fn run_tls_validate_challenge_impl(
    invocation: &TlsValidateChallengeInvocation,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    match built.runtime_plan.runtime.tls.mode {
        davenda_config::TlsMode::External => {
            return Err(CliRunError::execution(format!(
                "tls validate-challenge is unavailable for customer app `{}` because tls.mode is `external`",
                built.manifest.id
            )));
        }
        davenda_config::TlsMode::Manual => {
            return Err(CliRunError::execution(format!(
                "tls validate-challenge is unavailable for customer app `{}` because tls.mode is `manual`",
                built.manifest.id
            )));
        }
        _ => {}
    }

    let host = built
        .runtime_plan
        .runtime
        .tls_validation_host_with_secret_resolver(&EnvironmentSecretResolver)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build TLS host for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let bindings = customer_app_tls_bindings(&built)?;
    let validation = host
        .validate_challenge_for_bindings(bindings.clone())
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to validate TLS challenge setup for `{}`: {error}",
                built.manifest.id
            ))
        })?;

    let mut report = CommandReport::new(
        ["tls", "validate-challenge"],
        format!(
            "Validated TLS challenge setup for customer app `{}`",
            built.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "provider",
        "configured_challenge",
        "effective_challenge",
        "shared",
        "hot_reload",
        "hostnames",
    ])
    .map_err(report_build_error)?;
    report.push_row(
        ReportRow::new()
            .with_cell("provider", validation.provider.to_string())
            .map_err(report_build_error)?
            .with_cell(
                "configured_challenge",
                validation
                    .configured_challenge
                    .map(|challenge| challenge.to_string())
                    .unwrap_or_else(|| "none".to_string()),
            )
            .map_err(report_build_error)?
            .with_cell(
                "effective_challenge",
                validation
                    .effective_challenge
                    .map(|challenge| challenge.to_string())
                    .unwrap_or_else(|| "none".to_string()),
            )
            .map_err(report_build_error)?
            .with_cell(
                "shared",
                if validation.shared_across_nodes {
                    "yes"
                } else {
                    "no"
                },
            )
            .map_err(report_build_error)?
            .with_cell(
                "hot_reload",
                if validation.requires_hot_reload {
                    "required"
                } else {
                    "not_required"
                },
            )
            .map_err(report_build_error)?
            .with_cell(
                "hostnames",
                bindings
                    .iter()
                    .map(|binding| binding.hostname.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .map_err(report_build_error)?,
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "tls.validate.mode",
        format!(
            "mode={:?} provider={} configured_challenge={} effective_challenge={}",
            built.runtime_plan.runtime.tls.mode,
            validation.provider,
            validation
                .configured_challenge
                .map(|challenge| challenge.to_string())
                .unwrap_or_else(|| "none".to_string()),
            validation
                .effective_challenge
                .map(|challenge| challenge.to_string())
                .unwrap_or_else(|| "none".to_string())
        ),
    )?;
    for check in &validation.checks {
        let code = format!("tls.validate.{}", check.name);
        push_report_diagnostic(
            &mut report,
            if check.ok {
                DiagnosticSeverity::Info
            } else {
                DiagnosticSeverity::Error
            },
            &code,
            check.detail.clone(),
        )?;
    }

    Ok(report)
}

fn run_tls_renew(
    invocation: &TlsRenewInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    if !dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`tls renew` requires `--yes` unless `--dry-run` is used",
        ));
    }

    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    if built.runtime_plan.runtime.tls.mode == davenda_config::TlsMode::External {
        return Err(CliRunError::execution(format!(
            "tls renew is unavailable for customer app `{}` because tls.mode is `external`",
            built.manifest.id
        )));
    }
    let certificate_id =
        CertificateId::new(invocation.certificate_id.clone()).map_err(|error| {
            CliRunError::usage(format!(
                "invalid `--certificate` value `{}`: {error}",
                invocation.certificate_id
            ))
        })?;
    let replacement_certificate_id =
        CertificateId::new(invocation.replacement_certificate_id.clone()).map_err(|error| {
            CliRunError::usage(format!(
                "invalid `--replacement` value `{}`: {error}",
                invocation.replacement_certificate_id
            ))
        })?;
    let mut host = built.runtime_plan.runtime.tls_host().map_err(|error| {
        CliRunError::execution(format!(
            "failed to build TLS host for `{}`: {error}",
            built.manifest.id
        ))
    })?;
    let snapshot = host.status();
    let record = snapshot.inventory.record(&certificate_id).ok_or_else(|| {
        CliRunError::execution(format!(
            "TLS certificate `{}` is not present for customer app `{}`",
            certificate_id, built.manifest.id
        ))
    })?;

    let mut report = CommandReport::new(
        ["tls", "renew"],
        if dry_run {
            format!(
                "Planned TLS renewal for certificate `{}` on customer app `{}`",
                certificate_id, built.manifest.id
            )
        } else {
            format!(
                "Renewed TLS certificate `{}` on customer app `{}`",
                certificate_id, built.manifest.id
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns([
        "certificate",
        "replacement",
        "status",
        "hostnames",
        "not_after",
    ])
    .map_err(report_build_error)?;

    let replacement_status = if dry_run {
        "planned".to_string()
    } else {
        let renewed = host
            .renew_certificate(
                &certificate_id,
                replacement_certificate_id.clone(),
                TlsInstant::from_unix_seconds(unix_timestamp_now()?),
            )
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to renew TLS certificate `{}` for `{}`: {error}",
                    certificate_id, built.manifest.id
                ))
            })?;
        format!("{:?}", renewed.status).to_lowercase()
    };

    report.push_row(
        ReportRow::new()
            .with_cell("certificate", certificate_id.to_string())
            .map_err(report_build_error)?
            .with_cell("replacement", replacement_certificate_id.to_string())
            .map_err(report_build_error)?
            .with_cell("status", replacement_status)
            .map_err(report_build_error)?
            .with_cell(
                "hostnames",
                record
                    .bindings
                    .iter()
                    .map(|binding| binding.hostname.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
            .map_err(report_build_error)?
            .with_cell("not_after", record.not_after.to_string())
            .map_err(report_build_error)?,
    );

    Ok(report)
}

fn warm_cache_routes(
    built: &BuiltCustomerAppContext,
    routes: &[String],
    scope: &str,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    if scope != "public" {
        return Err(CliRunError::usage(
            "`cache warm` currently supports only `--scope public`",
        ));
    }

    let mut report = CommandReport::new(
        ["cache", "warm"],
        if dry_run {
            format!(
                "Planned cache warm for `{}` across {} route(s)",
                built.manifest.id,
                routes.len()
            )
        } else {
            format!(
                "Warmed cache for `{}` across {} route(s)",
                built.manifest.id,
                routes.len()
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns([
        "route",
        "route_name",
        "scope",
        "cache",
        "status",
        "cache_key",
    ])
    .map_err(report_build_error)?;

    let host = built.runtime_plan.runtime.config.seo.canonical_host.clone();
    let now = CacheInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                CliRunError::execution(format!("failed to calculate cache warm timestamp: {error}"))
            })?
            .as_secs(),
    );
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let mut cache_host = if dry_run {
        None
    } else {
        Some(built.runtime_plan.runtime.cache_host().map_err(|error| {
            CliRunError::execution(format!(
                "failed to build cache host for `{}`: {error}",
                built.manifest.id
            ))
        })?)
    };

    for route in routes {
        let request =
            RequestInput::new(HttpMethod::Get, host.as_str(), route).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to prepare cache warm request `{route}`: {error}"
                ))
            })?;
        let execution = built
            .runtime_plan
            .runtime
            .execute_request(request, cookie_secret, csrf_secret)
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to resolve cache warm route `{route}` for `{}`: {error}",
                    built.manifest.id
                ))
            })?;
        if execution.cache != CacheDisposition::Public {
            return Err(CliRunError::execution(format!(
                "cache warm route `{route}` resolved to `{}` cache disposition; only public routes are supported",
                cache_disposition_label(execution.cache)
            )));
        }
        let cache_key = execution
            .cache_plan
            .plan
            .application()
            .ok_or_else(|| {
                CliRunError::execution(format!(
                    "cache warm route `{route}` does not produce an application cache plan"
                ))
            })?
            .key()
            .to_string();
        let status = if dry_run {
            "planned".to_string()
        } else {
            let value = render_cache_warm_value(&built.runtime_plan.runtime, &execution)?;
            let cache_host = cache_host
                .as_mut()
                .expect("non-dry-run cache warm builds a cache host");
            let fill = cache_host
                .begin_fill(&execution, "platform:cache:warm")
                .ok_or_else(|| {
                    CliRunError::execution(format!(
                        "cache warm route `{route}` could not acquire an application cache fill"
                    ))
                })?;
            cache_host
                .store_execution(&execution, value, now)
                .ok_or_else(|| {
                    CliRunError::execution(format!(
                        "cache warm route `{route}` is not application-cacheable"
                    ))
                })?;
            cache_host.complete_fill(&fill).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to complete cache fill for route `{route}`: {error}"
                ))
            })?;
            "warmed".to_string()
        };

        report.push_row(
            ReportRow::new()
                .with_cell("route", route.clone())
                .map_err(report_build_error)?
                .with_cell("route_name", execution.route.route_name.clone())
                .map_err(report_build_error)?
                .with_cell("scope", scope.to_string())
                .map_err(report_build_error)?
                .with_cell("cache", cache_disposition_label(execution.cache))
                .map_err(report_build_error)?
                .with_cell("status", status)
                .map_err(report_build_error)?
                .with_cell("cache_key", cache_key)
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cache.routes",
        format!(
            "cache warm {} route(s) against host `{}` for customer app `{}`",
            routes.len(),
            host,
            built.manifest.id
        ),
    )?;

    Ok(report)
}

fn inspect_cache_route(
    built: &BuiltCustomerAppContext,
    route: &str,
) -> Result<CommandReport, CliRunError> {
    let (execution, cache_key) = resolve_cache_route_execution(built, route)?;
    let mut report = CommandReport::new(
        ["cache", "inspect"],
        format!(
            "Cache inspection for `{}` route `{route}`",
            built.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns([
        "route",
        "route_name",
        "scope",
        "cache",
        "cache_key",
        "lookup",
        "entry",
    ])
    .map_err(report_build_error)?;

    let application_plan = execution.cache_plan.plan.application();
    if application_plan.is_none() {
        report = report.with_status(ReportStatus::Warning);
        report.push_row(
            ReportRow::new()
                .with_cell("route", route.to_string())
                .map_err(report_build_error)?
                .with_cell("route_name", execution.route.route_name.clone())
                .map_err(report_build_error)?
                .with_cell("scope", "n/a")
                .map_err(report_build_error)?
                .with_cell("cache", cache_disposition_label(execution.cache))
                .map_err(report_build_error)?
                .with_cell("cache_key", "none")
                .map_err(report_build_error)?
                .with_cell("lookup", "not_cacheable")
                .map_err(report_build_error)?
                .with_cell("entry", "absent")
                .map_err(report_build_error)?,
        );
        return Ok(report);
    }

    let now = CacheInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to calculate cache inspection timestamp: {error}"
                ))
            })?
            .as_secs(),
    );
    if let Some(error) = cache_backend_availability_error(built) {
        report = report.with_status(ReportStatus::Warning);
        report.push_row(
            ReportRow::new()
                .with_cell("route", route.to_string())
                .map_err(report_build_error)?
                .with_cell("route_name", execution.route.route_name.clone())
                .map_err(report_build_error)?
                .with_cell(
                    "scope",
                    format!(
                        "{:?}",
                        application_plan
                            .expect("checked above")
                            .scope()
                            .visibility()
                    )
                    .to_lowercase(),
                )
                .map_err(report_build_error)?
                .with_cell("cache", cache_disposition_label(execution.cache))
                .map_err(report_build_error)?
                .with_cell("cache_key", cache_key.clone())
                .map_err(report_build_error)?
                .with_cell("lookup", "backend_unavailable")
                .map_err(report_build_error)?
                .with_cell("entry", "unknown")
                .map_err(report_build_error)?,
        );
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "cache.inspect.backend",
            format!(
                "cache backend is unavailable for live lookup, but the resolved cache key is still `{cache_key}`: {error}"
            ),
        )?;
        return Ok(report);
    }
    let mut cache_host = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        built.runtime_plan.runtime.cache_host()
    })) {
        Ok(Ok(host)) => host,
        Ok(Err(error)) => {
            report = report.with_status(ReportStatus::Warning);
            report.push_row(
                ReportRow::new()
                    .with_cell("route", route.to_string())
                    .map_err(report_build_error)?
                    .with_cell("route_name", execution.route.route_name.clone())
                    .map_err(report_build_error)?
                    .with_cell(
                        "scope",
                        format!(
                            "{:?}",
                            application_plan
                                .expect("checked above")
                                .scope()
                                .visibility()
                        )
                        .to_lowercase(),
                    )
                    .map_err(report_build_error)?
                    .with_cell("cache", cache_disposition_label(execution.cache))
                    .map_err(report_build_error)?
                    .with_cell("cache_key", cache_key.clone())
                    .map_err(report_build_error)?
                    .with_cell("lookup", "backend_unavailable")
                    .map_err(report_build_error)?
                    .with_cell("entry", "unknown")
                    .map_err(report_build_error)?,
            );
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Warning,
                "cache.inspect.backend",
                format!(
                    "cache backend is unavailable for live lookup, but the resolved cache key is still `{cache_key}`: {error}"
                ),
            )?;
            return Ok(report);
        }
        Err(_) => {
            report = report.with_status(ReportStatus::Warning);
            report.push_row(
                ReportRow::new()
                    .with_cell("route", route.to_string())
                    .map_err(report_build_error)?
                    .with_cell("route_name", execution.route.route_name.clone())
                    .map_err(report_build_error)?
                    .with_cell(
                        "scope",
                        format!(
                            "{:?}",
                            application_plan
                                .expect("checked above")
                                .scope()
                                .visibility()
                        )
                        .to_lowercase(),
                    )
                    .map_err(report_build_error)?
                    .with_cell("cache", cache_disposition_label(execution.cache))
                    .map_err(report_build_error)?
                    .with_cell("cache_key", cache_key.clone())
                    .map_err(report_build_error)?
                    .with_cell("lookup", "backend_unavailable")
                    .map_err(report_build_error)?
                    .with_cell("entry", "unknown")
                    .map_err(report_build_error)?,
            );
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Warning,
                "cache.inspect.backend",
                format!(
                    "cache backend panicked during initialization, but the resolved cache key is still `{cache_key}`"
                ),
            )?;
            return Ok(report);
        }
    };
    let lookup = cache_host
        .lookup_execution(&execution, now)
        .ok_or_else(|| {
            CliRunError::execution(format!(
                "cache inspect route `{route}` does not produce an application cache plan"
            ))
        })?;
    let lookup_label = format!("{:?}", lookup.state).to_lowercase();
    let entry_label = if lookup.entry.is_some() {
        "present"
    } else {
        "absent"
    };
    if lookup.entry.is_none() {
        report = report.with_status(ReportStatus::Warning);
    }
    report.push_row(
        ReportRow::new()
            .with_cell("route", route.to_string())
            .map_err(report_build_error)?
            .with_cell("route_name", execution.route.route_name.clone())
            .map_err(report_build_error)?
            .with_cell(
                "scope",
                format!(
                    "{:?}",
                    application_plan
                        .expect("checked above")
                        .scope()
                        .visibility()
                )
                .to_lowercase(),
            )
            .map_err(report_build_error)?
            .with_cell("cache", cache_disposition_label(execution.cache))
            .map_err(report_build_error)?
            .with_cell("cache_key", cache_key.clone())
            .map_err(report_build_error)?
            .with_cell("lookup", lookup_label.clone())
            .map_err(report_build_error)?
            .with_cell("entry", entry_label)
            .map_err(report_build_error)?,
    );
    if let Some(entry) = lookup.entry {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "cache.inspect.entry",
            format!(
                "ttl={}s tags={} stale_while_revalidate={}s needs_revalidation={}",
                entry.freshness.ttl_seconds(),
                entry
                    .tags
                    .header_value()
                    .unwrap_or_else(|| "none".to_string()),
                entry
                    .freshness
                    .stale_while_revalidate_seconds()
                    .unwrap_or_default(),
                lookup.needs_revalidation
            ),
        )?;
    } else {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "cache.inspect.miss",
            format!("no cache entry is currently stored for `{cache_key}`"),
        )?;
    }

    Ok(report)
}

fn invalidate_cache_tags(
    built: &BuiltCustomerAppContext,
    tags: &[String],
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let invalidation_tags = tags
        .iter()
        .cloned()
        .map(InvalidationTag::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| CliRunError::usage(format!("invalid cache tag: {error}")))?;
    let invalidation = InvalidationSet::from_tags(invalidation_tags.clone());
    let mut report = CommandReport::new(
        ["cache", "invalidate"],
        if dry_run {
            format!(
                "Planned cache invalidation for `{}` across {} tag(s)",
                built.manifest.id,
                tags.len()
            )
        } else {
            format!(
                "Invalidated cache for `{}` across {} tag(s)",
                built.manifest.id,
                tags.len()
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns(["tag", "status"])
    .map_err(report_build_error)?;

    for tag in tags {
        report.push_row(
            ReportRow::new()
                .with_cell("tag", tag.clone())
                .map_err(report_build_error)?
                .with_cell("status", if dry_run { "planned" } else { "invalidated" })
                .map_err(report_build_error)?,
        );
    }

    let invalidated = if dry_run {
        Vec::new()
    } else {
        if let Some(error) = cache_backend_availability_error(built) {
            return Err(CliRunError::execution(error));
        }
        let mut cache_host = built.runtime_plan.runtime.cache_host().map_err(|error| {
            CliRunError::execution(format!(
                "failed to build cache host for `{}`: {error}",
                built.manifest.id
            ))
        })?;
        cache_host.invalidate(&invalidation)
    };

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cache.invalidate.tags",
        format!("cache invalidation tags: {}", tags.join(", ")),
    )?;
    push_report_diagnostic(
        &mut report,
        if dry_run || !invalidated.is_empty() {
            DiagnosticSeverity::Info
        } else {
            DiagnosticSeverity::Warning
        },
        "cache.invalidate.keys",
        if dry_run {
            "dry-run only; no cache entries were removed".to_string()
        } else if invalidated.is_empty() {
            "no matching cache entries were present for the requested tags".to_string()
        } else {
            format!(
                "invalidated {} cache key(s): {}",
                invalidated.len(),
                invalidated
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        },
    )?;

    Ok(report)
}

fn resolve_cache_route_execution(
    built: &BuiltCustomerAppContext,
    route: &str,
) -> Result<(davenda_runtime::RequestExecution, String), CliRunError> {
    resolve_cache_route_execution_for_principal(built, route, None)
}

fn resolve_cache_route_execution_for_principal(
    built: &BuiltCustomerAppContext,
    route: &str,
    principal_id: Option<&str>,
) -> Result<(davenda_runtime::RequestExecution, String), CliRunError> {
    let host = built.runtime_plan.runtime.config.seo.canonical_host.clone();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let mut candidate_routes = vec![route.to_string()];
    if let Some(localized_route) = localized_cache_route_candidate(built, route) {
        if !candidate_routes
            .iter()
            .any(|candidate| candidate == &localized_route)
        {
            candidate_routes.push(localized_route);
        }
    }

    let mut last_error = None;
    for candidate in &candidate_routes {
        let mut request =
            RequestInput::new(HttpMethod::Get, host.as_str(), candidate).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to prepare cache route request `{candidate}`: {error}"
                ))
            })?;
        if let Some(principal_id) = principal_id {
            request = request.with_principal(principal_id.to_string());
        }
        match built
            .runtime_plan
            .runtime
            .execute_request(request, cookie_secret, csrf_secret)
        {
            Ok(execution) => {
                let cache_key = execution
                    .cache_plan
                    .plan
                    .application()
                    .map(|plan| plan.key().to_string())
                    .unwrap_or_else(|| "none".to_string());
                return Ok((execution, cache_key));
            }
            Err(error) => last_error = Some((candidate.clone(), error)),
        }
    }

    let attempted = candidate_routes.join(", ");
    let (failed_route, error) =
        last_error.expect("cache route execution should attempt at least one route");
    Err(CliRunError::execution(format!(
        "failed to resolve cache route `{route}` for `{}` after trying [{attempted}] (last attempt `{failed_route}`): {error}",
        built.manifest.id
    )))
}

fn normalize_expected_cache_headers(
    headers: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .collect()
}

fn compare_expected_cache_headers(
    expected: &BTreeMap<String, String>,
    observed: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut mismatches = Vec::new();
    for (name, expected_value) in expected {
        match observed.get(name) {
            Some(actual) if actual == expected_value => {}
            Some(actual) => mismatches.push(format!(
                "{name} expected `{expected_value}` but observed `{actual}`"
            )),
            None => mismatches.push(format!(
                "{name} expected `{expected_value}` but was missing"
            )),
        }
    }
    mismatches
}

fn observed_public_cache_policy(
    headers: &BTreeMap<String, String>,
) -> Option<BTreeMap<String, String>> {
    let cache_control = headers.get("cache-control")?;
    let lower = cache_control.to_ascii_lowercase();
    if !lower.contains("public") || lower.contains("private") || lower.contains("no-store") {
        return None;
    }

    let mut policy = BTreeMap::from([("cache-control".to_string(), cache_control.clone())]);
    if let Some(value) = headers.get("surrogate-key") {
        policy.insert("surrogate-key".to_string(), value.clone());
    }
    if let Some(value) = headers.get("vary") {
        policy.insert("vary".to_string(), value.clone());
    }
    Some(policy)
}

fn localized_cache_route_candidate(built: &BuiltCustomerAppContext, route: &str) -> Option<String> {
    let i18n = &built.runtime_plan.runtime.config.i18n;
    if !i18n.localized_routes || route == "/" {
        return None;
    }
    let trimmed = route.trim_start_matches('/');
    let first_segment = trimmed.split('/').next().unwrap_or_default();
    if i18n
        .supported_locales
        .iter()
        .any(|locale| locale == first_segment)
    {
        return None;
    }
    Some(format!(
        "/{}/{}",
        i18n.default_locale.trim_matches('/'),
        trimmed
    ))
}

fn cache_backend_availability_error(built: &BuiltCustomerAppContext) -> Option<String> {
    match built.runtime_plan.runtime.config.cache.l2 {
        Some(davenda_config::DistributedCache::Redis) if std::env::var("REDIS_URL").is_err() => {
            Some("redis cache backend requires REDIS_URL to be set".to_string())
        }
        Some(davenda_config::DistributedCache::Valkey)
            if std::env::var("VALKEY_URL").is_err() && std::env::var("REDIS_URL").is_err() =>
        {
            Some("valkey cache backend requires VALKEY_URL or REDIS_URL to be set".to_string())
        }
        _ => None,
    }
}

fn render_cache_warm_value(
    runtime: &davenda_runtime::RuntimePlan,
    execution: &davenda_runtime::RequestExecution,
) -> Result<String, CliRunError> {
    match &execution.response {
        HandlerResponse::Page(page) => {
            runtime
                .render_page_response(execution, page, None)
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to render cache warm page `{}`: {error}",
                        execution.route.route_name
                    ))
                })
        }
        HandlerResponse::Fragment(fragment) => runtime
            .render_fragment_response(execution, fragment)
            .map_err(|error| {
                CliRunError::execution(format!(
                    "failed to render cache warm fragment `{}`: {error}",
                    execution.route.route_name
                ))
            }),
        HandlerResponse::Json(json) => serde_json::to_string(&json.payload).map_err(|error| {
            CliRunError::execution(format!(
                "failed to serialize cache warm json `{}`: {error}",
                execution.route.route_name
            ))
        }),
        HandlerResponse::Redirect(redirect) => Ok(redirect.location.clone()),
        HandlerResponse::File(file) => Ok(format!("{}:{}", file.content_type, file.logical_path)),
    }
}

fn cache_disposition_label(disposition: CacheDisposition) -> &'static str {
    match disposition {
        CacheDisposition::Public => "public",
        CacheDisposition::Private => "private",
        CacheDisposition::Uncacheable => "uncacheable",
    }
}

fn run_storage_inspect(
    invocation: &StorageInspectInvocation,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(&invocation.config_path, true)?;
    let runtime = &built.runtime_plan.runtime;
    let storage_host = runtime.storage_host();
    let topology = storage_host.planner.topology().clone();
    let object_store_result = runtime.object_store_client_config(&EnvironmentSecretResolver);
    let object_store_status = match &object_store_result {
        Ok(Some(config)) => format!("resolved bucket={} region={}", config.bucket, config.region),
        Ok(None) => "not configured".to_string(),
        Err(error) => format!("invalid: {error}"),
    };
    let object_store_backend = match &topology.object_store {
        Some(target) => format!("{:?}", target.backend_kind()),
        None => "none".to_string(),
    };
    let cdn_base_url = built
        .runtime_plan
        .runtime
        .config
        .assets
        .cdn_base_url
        .clone()
        .unwrap_or_else(|| "none".to_string());
    let mut report = CommandReport::new(
        ["storage", "inspect"],
        format!(
            "Inspected storage topology for customer app `{}`",
            built.manifest.id
        ),
    )
    .map_err(report_build_error)?
    .with_columns(["section", "key", "value", "detail"])
    .map_err(report_build_error)?;

    for (section, key, value, detail) in [
        (
            "topology",
            "default_class",
            storage_class_label(topology.default_class).to_string(),
            "default storage class used when no explicit rule is requested".to_string(),
        ),
        (
            "topology",
            "deployment",
            storage_deployment_label(topology.deployment).to_string(),
            "deployment scope used for durable and single-node planning".to_string(),
        ),
        (
            "topology",
            "single_node_escape_hatch",
            format!("{:?}", topology.single_node_escape_hatch),
            "whether explicit local-only storage remains available".to_string(),
        ),
        (
            "topology",
            "local_root",
            topology.local_root.clone(),
            "local storage root for single-node and escape-hatch writes".to_string(),
        ),
        (
            "object_store",
            "backend",
            object_store_backend,
            "resolved scalable object-store backend kind from config".to_string(),
        ),
        (
            "object_store",
            "status",
            object_store_status,
            "live object-store secret and client resolution state".to_string(),
        ),
        (
            "delivery",
            "cdn_base_url",
            cdn_base_url,
            "base URL used for public CDN delivery and manifest publication".to_string(),
        ),
    ] {
        report.push_row(
            ReportRow::new()
                .with_cell("section", section)
                .map_err(report_build_error)?
                .with_cell("key", key)
                .map_err(report_build_error)?
                .with_cell("value", value)
                .map_err(report_build_error)?
                .with_cell("detail", detail)
                .map_err(report_build_error)?,
        );
    }

    match object_store_result {
        Ok(Some(config)) => {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Info,
                "storage.inspect.object_store",
                format!(
                    "object store resolved for bucket `{}` in region `{}`",
                    config.bucket, config.region
                ),
            )?;
        }
        Ok(None) => {
            report = report.with_status(ReportStatus::Warning);
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Warning,
                "storage.inspect.object_store",
                format!(
                    "customer app `{}` has no configured object store; scalable asset publication is unavailable",
                    built.manifest.id
                ),
            )?;
        }
        Err(error) => {
            report = report.with_status(ReportStatus::Unsafe);
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Error,
                "storage.inspect.object_store",
                format!(
                    "failed to resolve object-store backend for `{}`: {error}",
                    built.manifest.id
                ),
            )?;
        }
    }

    Ok(report)
}

fn run_storage_verify(
    config_path: &Path,
    verify_policy: bool,
) -> Result<CommandReport, CliRunError> {
    let built = build_customer_app_runtime_context(config_path, true)?;
    let runtime = &built.runtime_plan.runtime;
    let storage_host = runtime.storage_host();

    let mut report = CommandReport::new(
        ["storage", "verify"],
        if verify_policy {
            format!(
                "Verified storage policy planning for customer app `{}`",
                built.manifest.id
            )
        } else {
            format!(
                "Verified storage topology and backend resolution for customer app `{}`",
                built.manifest.id
            )
        },
    )
    .map_err(report_build_error)?
    .with_columns([
        "class",
        "logical_path",
        "policy",
        "durable_store",
        "scope",
        "backend",
        "locator",
        "result",
        "detail",
    ])
    .map_err(report_build_error)?;

    let object_store_result = runtime.object_store_client_config(&EnvironmentSecretResolver);
    match &object_store_result {
        Ok(Some(config)) => {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Info,
                "storage.object_store.resolved",
                format!(
                    "resolved object-store backend for bucket `{}` in region `{}`",
                    config.bucket, config.region
                ),
            )?;
        }
        Ok(None) => {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Warning,
                "storage.object_store.missing",
                format!(
                    "customer app `{}` has no configured object-store backend; public asset and shared private writes cannot be verified as scalable",
                    built.manifest.id
                ),
            )?;
            report = report.with_status(ReportStatus::Warning);
        }
        Err(error) => {
            push_report_diagnostic(
                &mut report,
                DiagnosticSeverity::Error,
                "storage.object_store.invalid",
                format!(
                    "failed to resolve object-store backend for `{}`: {error}",
                    built.manifest.id
                ),
            )?;
            report = report.with_status(ReportStatus::Unsafe);
        }
    }

    let checks = [
        (
            StorageClass::PublicAsset,
            "verify/public-asset.bin",
            false,
            "verify platform-managed public deployment assets",
        ),
        (
            StorageClass::PublicUpload,
            "verify/public-upload.bin",
            false,
            "verify public uploads use scalable storage",
        ),
        (
            StorageClass::PrivateShared,
            "verify/private-shared.bin",
            false,
            "verify private shared assets use durable shared storage",
        ),
        (
            StorageClass::LocalOnlySensitive,
            "verify/local-only-sensitive.bin",
            true,
            "verify local-only sensitive assets stay on the single-node escape hatch",
        ),
    ];

    for (class, logical_path, single_node_only, description) in checks {
        let request = StoragePlanRequest::new(logical_path).with_storage_class(class);
        let plan_result = if single_node_only {
            storage_host.plan_single_node_escape_hatch_write(request)
        } else {
            storage_host.plan_write(request)
        };

        match plan_result {
            Ok(plan) => {
                let primary = plan.primary_write_target();
                report.push_row(
                    ReportRow::new()
                        .with_cell("class", storage_class_label(class))
                        .map_err(report_build_error)?
                        .with_cell("logical_path", logical_path)
                        .map_err(report_build_error)?
                        .with_cell("policy", storage_policy_label(&plan))
                        .map_err(report_build_error)?
                        .with_cell("durable_store", format!("{:?}", plan.durable_store))
                        .map_err(report_build_error)?
                        .with_cell("scope", format!("{:?}", plan.deployment_scope))
                        .map_err(report_build_error)?
                        .with_cell(
                            "backend",
                            primary
                                .map(|target| format!("{:?}", target.backend))
                                .unwrap_or_else(|| "none".to_string()),
                        )
                        .map_err(report_build_error)?
                        .with_cell(
                            "locator",
                            primary
                                .map(|target| target.locator.clone())
                                .unwrap_or_else(|| "none".to_string()),
                        )
                        .map_err(report_build_error)?
                        .with_cell("result", "ok")
                        .map_err(report_build_error)?
                        .with_cell("detail", description)
                        .map_err(report_build_error)?,
                );
            }
            Err(error) => {
                if report.status == ReportStatus::Ok {
                    report = report.with_status(ReportStatus::Unsafe);
                }
                report.push_row(
                    ReportRow::new()
                        .with_cell("class", storage_class_label(class))
                        .map_err(report_build_error)?
                        .with_cell("logical_path", logical_path)
                        .map_err(report_build_error)?
                        .with_cell("policy", "unavailable")
                        .map_err(report_build_error)?
                        .with_cell("durable_store", "unavailable")
                        .map_err(report_build_error)?
                        .with_cell("scope", "unavailable")
                        .map_err(report_build_error)?
                        .with_cell("backend", "unavailable")
                        .map_err(report_build_error)?
                        .with_cell("locator", "unavailable")
                        .map_err(report_build_error)?
                        .with_cell("result", "invalid")
                        .map_err(report_build_error)?
                        .with_cell("detail", error.to_string())
                        .map_err(report_build_error)?,
                );
            }
        }
    }

    Ok(report)
}

fn run_import_manifest(
    invocation: &ImportRunInvocation,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let manifest = ImportManifest::from_file(&invocation.manifest_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load import manifest from `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let manifest_root = invocation
        .manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    manifest.validate_at(manifest_root).map_err(|error| {
        CliRunError::execution(format!(
            "failed to validate import manifest `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let plan = manifest.plan().map_err(|error| {
        CliRunError::execution(format!(
            "failed to plan import manifest `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let import_runtime = build_import_runtime_context(manifest_root, &manifest)?;
    if plan.publication_mode == PublicationMode::PublishValidated && import_runtime.is_none() {
        return Err(CliRunError::execution(format!(
            "publish-validated import manifest `{}` requires a `[target]` runtime configuration",
            invocation.manifest_path.display()
        )));
    }

    if dry_run {
        return plan.command_report().map_err(|error| {
            CliRunError::execution(format!(
                "failed to render import plan `{}`: {error}",
                invocation.manifest_path.display()
            ))
        });
    }

    let journal_path = import_journal_path(&invocation.manifest_path, &manifest.run_id);
    let execution = if let Some(runtime) = import_runtime.as_ref() {
        let storage_host = runtime.storage_host.clone();
        let default_locale = runtime.built.manifest.default_locale.to_string();
        let publish_validated = plan.publication_mode == PublicationMode::PublishValidated;
        let requires_live_data = publish_validated
            && plan.ordered_importers.iter().any(|importer| {
                matches!(
                    importer.resource_kind.as_str(),
                    "page" | "event" | "user" | "membership_tier" | "subscription"
                )
            });
        let requires_live_auth = publish_validated
            && plan
                .ordered_importers
                .iter()
                .any(|importer| matches!(importer.resource_kind.as_str(), "user" | "subscription"));
        let requires_live_user_auth_mapping = publish_validated
            && plan
                .ordered_importers
                .iter()
                .any(|importer| importer.resource_kind == "user");
        let data_runtime = runtime.built.runtime_plan.runtime.data.clone();
        let mut data_client = None;
        let tokio_runtime = if requires_live_data || requires_live_auth {
            Some(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| {
                        CliRunError::execution(format!(
                            "failed to start runtime for live import materialization: {error}"
                        ))
                    })?,
            )
        } else {
            None
        };
        let mut auth_context = match (requires_live_auth, tokio_runtime.as_ref()) {
            (true, Some(tokio_runtime)) => Some(build_import_auth_context(
                runtime,
                manifest_root,
                &manifest,
                requires_live_user_auth_mapping,
                tokio_runtime,
            )?),
            _ => None,
        };
        plan.execute_with_handler(
            manifest_root,
            &journal_path,
            |importer, _, manifest_root, staged_record| {
                match importer.resource_kind.as_str() {
                    "asset" => {
                        materialize_asset_record(
                            &storage_host,
                            manifest.asset_storage_default,
                            manifest_root,
                            staged_record,
                        )?;
                    }
                    "page" if publish_validated => {
                        let client = ensure_import_data_client(&data_runtime, &mut data_client)?;
                        materialize_page_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &client,
                            &default_locale,
                            staged_record,
                        )?;
                    }
                    "event" if publish_validated => {
                        let client = ensure_import_data_client(&data_runtime, &mut data_client)?;
                        materialize_event_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &client,
                            staged_record,
                        )?;
                    }
                    "membership_tier" if publish_validated => {
                        let client = ensure_import_data_client(&data_runtime, &mut data_client)?;
                        materialize_membership_tier_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &client,
                            staged_record,
                        )?;
                    }
                    "subscription" if publish_validated => {
                        let client = ensure_import_data_client(&data_runtime, &mut data_client)?;
                        let auth_context = auth_context.as_mut().expect(
                            "publish-validated subscription imports build a live auth context",
                        );
                        materialize_subscription_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &data_runtime,
                            &client,
                            auth_context,
                            staged_record,
                        )?;
                    }
                    "user" if publish_validated => {
                        let client = ensure_import_data_client(&data_runtime, &mut data_client)?;
                        let auth_context = auth_context
                            .as_mut()
                            .expect("publish-validated imports build a live auth context");
                        materialize_user_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &data_runtime,
                            &client,
                            auth_context,
                            staged_record,
                        )?;
                    }
                    _ => {}
                }
                Ok(())
            },
        )
    } else {
        plan.execute(manifest_root, &journal_path)
    }
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to execute import manifest `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let mut report = execution.command_report().map_err(|error| {
        CliRunError::execution(format!(
            "failed to render import execution `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    if let Some(runtime) = import_runtime.as_ref() {
        materialize_import_assets(&mut report, manifest_root, runtime, &execution)?;
        materialize_import_pages(&mut report, manifest_root, runtime, &execution)?;
        materialize_import_events(&mut report, manifest_root, runtime, &execution)?;
        materialize_import_users(&mut report, manifest_root, runtime, &execution)?;
        materialize_import_memberships(&mut report, manifest_root, runtime, &execution)?;
    }
    Ok(report)
}

struct EvaluatedImportCutover {
    manifest: ImportManifest,
    cutover: davenda_import::ImportCutover,
    runtime: BuiltImportRuntimeContext,
    config_path: PathBuf,
    cutover_plan: CutoverPlan,
    verification_support: CutoverVerificationSupport,
    report: CommandReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerificationRouteProbe {
    name: String,
    method: HttpMethod,
    path: String,
    auth: RouteAuthGate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerificationWebhookProbe {
    extension_id: String,
    handler_id: String,
    source: String,
    event: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CutoverVerificationSupport {
    fragment_probe: Option<VerificationRouteProbe>,
    auth_probe: Option<VerificationRouteProbe>,
    transactional_probes: Vec<VerificationRouteProbe>,
    webhook_probes: Vec<VerificationWebhookProbe>,
}

const CLOUDFLARE_API_BASE_URL_ENV: &str = "DAVENDA_CLOUDFLARE_API_BASE_URL";
const CUTOVER_CLOUDFLARE_SECRET_ENV: &str = "DAVENDA_CUTOVER_CLOUDFLARE_SECRET";
const CUTOVER_SYNTHETIC_SESSION_ENV: &str = "DAVENDA_CUTOVER_ALLOW_SYNTHETIC_SESSION";

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloudflareDnsSwitchRequest {
    zone_id: String,
    target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloudflareTrafficTargetSwitchRequest {
    zone_id: String,
    resource_id: String,
    target: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CloudflareSecretPayload {
    cloudflare_api_token: Option<String>,
    cloudflare_service_key: Option<String>,
    cloudflare_email: Option<String>,
    cloudflare_api_key: Option<String>,
}

#[derive(Debug, Clone)]
struct CloudflareCredentials {
    raw: String,
    payload: CloudflareSecretPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CloudflareError {
    #[serde(default)]
    code: Option<u64>,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CloudflareResponseEnvelope<T> {
    success: bool,
    #[serde(default)]
    errors: Vec<CloudflareError>,
    result: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CloudflareDnsRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    #[serde(default)]
    proxied: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct CloudflareDnsRecordUpdate<'a> {
    #[serde(rename = "type")]
    record_type: &'a str,
    name: &'a str,
    content: &'a str,
    ttl: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxied: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CloudflareLoadBalancer {
    id: String,
    #[serde(default)]
    default_pools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CloudflareLoadBalancerUpdate<'a> {
    default_pools: Vec<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CloudflareOriginRule {
    id: String,
    origin: String,
}

#[derive(Debug, Clone, Serialize)]
struct CloudflareOriginRuleUpdate<'a> {
    origin: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CloudflareRoutingRule {
    id: String,
    service: String,
}

#[derive(Debug, Clone, Serialize)]
struct CloudflareRoutingRuleUpdate<'a> {
    service: &'a str,
}

fn run_import_cutover(invocation: &ImportCutoverInvocation) -> Result<CommandReport, CliRunError> {
    let evaluated = evaluate_import_cutover(invocation)?;
    let actions = [
        invocation.apply,
        invocation.switch,
        invocation.observe,
        invocation.rollback,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count();
    if actions > 1 {
        return Err(CliRunError::usage(
            "`import cutover` accepts only one of `--apply`, `--switch`, `--observe`, or `--rollback`",
        ));
    }
    if invocation.rollback {
        return rollback_import_cutover(invocation, &evaluated);
    }
    if invocation.switch {
        return switch_import_cutover(invocation, &evaluated);
    }
    if invocation.observe {
        return observe_import_cutover(invocation, &evaluated);
    }
    if !invocation.apply {
        return Ok(evaluated.report);
    }

    if !invocation.confirmed {
        return Err(CliRunError::usage(
            "`import cutover --apply` requires `--yes`",
        ));
    }
    if evaluated.cutover.freeze_legacy_writes && !invocation.legacy_freeze_confirmed {
        return Err(CliRunError::usage(
            "`import cutover --apply` requires `--legacy-freeze-confirmed` when the manifest freezes legacy writes",
        ));
    }
    if !cutover_preflight_ready(&evaluated.cutover_plan) {
        return Err(CliRunError::execution(format!(
            "cutover `{}` is not executable yet; rerun `platform import cutover {}` to inspect the blocking readiness checks",
            evaluated.manifest.run_id,
            invocation.manifest_path.display()
        )));
    }

    let journal_path = cutover_journal_path(&invocation.manifest_path, &evaluated.manifest.run_id);
    let expected_steps = cutover_steps(&evaluated.cutover)?;
    let mut journal = CutoverExecutionJournal::load(
        &journal_path,
        &evaluated.manifest.run_id,
        evaluated.manifest.customer_app_id.as_str(),
        expected_steps,
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to load cutover journal for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    if evaluated.cutover.freeze_legacy_writes {
        journal.confirm_freeze();
        save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;
    }

    run_cutover_step(
        &mut journal,
        &journal_path,
        &evaluated.manifest.run_id,
        "final.import",
        || {
            let report = run_import_manifest(
                &ImportRunInvocation {
                    manifest_path: invocation.manifest_path.clone(),
                },
                false,
            )?;
            if report.status != ReportStatus::Ok {
                return Err(CliRunError::execution(format!(
                    "final import for `{}` must finish without staged or failed records",
                    evaluated.manifest.run_id
                )));
            }
            Ok(report.summary.clone())
        },
    )?;

    if evaluated.cutover.requires_storage_validation {
        run_cutover_step(
            &mut journal,
            &journal_path,
            &evaluated.manifest.run_id,
            "storage.verify",
            || {
                let report = run_storage_verify(&evaluated.config_path, true)?;
                if report.status != ReportStatus::Ok {
                    return Err(CliRunError::execution(format!(
                        "storage verification for `{}` is not green",
                        evaluated.manifest.run_id
                    )));
                }
                Ok(report.summary.clone())
            },
        )?;
    }

    if evaluated.cutover.requires_assets_publish {
        run_cutover_step(
            &mut journal,
            &journal_path,
            &evaluated.manifest.run_id,
            "assets.publish",
            || {
                let report = run_assets_publish(
                    &AssetsPublishInvocation {
                        config_path: evaluated.config_path.clone(),
                        confirmed: true,
                    },
                    false,
                )?;
                if report.status != ReportStatus::Ok {
                    return Err(CliRunError::execution(format!(
                        "asset publication for `{}` is not green",
                        evaluated.manifest.run_id
                    )));
                }
                Ok(report.summary.clone())
            },
        )?;
    }

    if evaluated.cutover.requires_migrate_apply {
        run_cutover_step(
            &mut journal,
            &journal_path,
            &evaluated.manifest.run_id,
            "migrate.apply",
            || {
                let report = run_migrate_apply(
                    &MigrateApplyInvocation {
                        config_path: evaluated.config_path.clone(),
                        confirmed: true,
                    },
                    false,
                )?;
                if report.status != ReportStatus::Ok {
                    return Err(CliRunError::execution(format!(
                        "migrate apply for `{}` is not green",
                        evaluated.manifest.run_id
                    )));
                }
                Ok(report.summary.clone())
            },
        )?;
    }

    if evaluated.cutover.requires_cache_warm {
        let routes = evaluated
            .manifest
            .verification
            .as_ref()
            .map(|verification| verification.sample_routes.clone())
            .unwrap_or_default();
        run_cutover_step(
            &mut journal,
            &journal_path,
            &evaluated.manifest.run_id,
            "cache.warm",
            || {
                if routes.is_empty() {
                    return Err(CliRunError::execution(
                        "cutover cache warm requires verification.sample_routes".to_string(),
                    ));
                }
                let report = warm_cache_routes(&evaluated.runtime.built, &routes, "public", false)?;
                if report.status != ReportStatus::Ok {
                    return Err(CliRunError::execution(format!(
                        "cache warm for `{}` is not green",
                        evaluated.manifest.run_id
                    )));
                }
                Ok(report.summary.clone())
            },
        )?;
    }

    run_cutover_step(
        &mut journal,
        &journal_path,
        &evaluated.manifest.run_id,
        "cutover.readiness",
        || {
            let refreshed = evaluate_import_cutover(invocation)?;
            if refreshed.report.status != ReportStatus::Ok {
                return Err(CliRunError::execution(format!(
                    "cutover `{}` is still not fully ready after executing the owned preparation steps",
                    refreshed.manifest.run_id
                )));
            }
            Ok(refreshed.report.summary.clone())
        },
    )?;

    journal.mark_prepared();
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;
    let mut report = journal.command_report().map_err(|error| {
        CliRunError::execution(format!(
            "failed to render cutover execution report for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    report.summary = format!(
        "Cutover preparation for import run `{}` into customer app `{}` is prepared",
        evaluated.manifest.run_id, evaluated.manifest.customer_app_id
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.journal",
        format!("cutover journal persisted at `{}`", journal_path.display()),
    )?;
    Ok(report)
}

fn switch_import_cutover(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CommandReport, CliRunError> {
    if invocation.switch_plan_path.is_some() {
        return Err(CliRunError::usage(
            "`import cutover --switch` does not yet consume `--switch-plan`; use the provider-managed switch flags for the declared switch method",
        ));
    }
    if !invocation.dry_run && !invocation.confirmed {
        return Err(CliRunError::usage(
            "`import cutover --switch` requires `--yes`",
        ));
    }
    let base_url = invocation.base_url.as_ref().ok_or_else(|| {
        CliRunError::usage("`import cutover --switch` requires `--base-url <url>`")
    })?;

    let journal_path = cutover_journal_path(&invocation.manifest_path, &evaluated.manifest.run_id);
    let expected_steps = cutover_steps(&evaluated.cutover)?;
    let mut journal = CutoverExecutionJournal::load(
        &journal_path,
        &evaluated.manifest.run_id,
        evaluated.manifest.customer_app_id.as_str(),
        expected_steps,
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to load cutover journal for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;

    match journal.state {
        davenda_import::CutoverExecutionState::Prepared
        | davenda_import::CutoverExecutionState::SwitchConfirmed
        | davenda_import::CutoverExecutionState::Observing
        | davenda_import::CutoverExecutionState::ObservationPassed
        | davenda_import::CutoverExecutionState::RollbackRequired => {}
        _ => {
            return Err(CliRunError::execution(format!(
                "cutover `{}` must be prepared with `platform import cutover {} --apply --yes` before switch confirmation",
                evaluated.manifest.run_id,
                invocation.manifest_path.display()
            )));
        }
    }

    let switch_execution = execute_cutover_switch(invocation, evaluated)?;
    if invocation.dry_run {
        return planned_cutover_switch_report(evaluated, base_url, &switch_execution);
    }
    let switch_detail = render_cutover_switch_detail(&switch_execution, base_url);
    let switched_at = unix_timestamp_now()?;
    run_cutover_step(
        &mut journal,
        &journal_path,
        &evaluated.manifest.run_id,
        "switch.confirmed",
        || Ok(switch_detail.clone()),
    )?;
    journal.record_switch_execution(switch_execution.clone());
    journal.confirm_switch(base_url.clone(), switched_at);
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;

    let mut report = journal.command_report().map_err(|error| {
        CliRunError::execution(format!(
            "failed to render cutover switch report for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    report.summary = format!(
        "Cutover switch for import run `{}` executed against `{}`",
        evaluated.manifest.run_id, base_url
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.switch",
        render_cutover_switch_detail(&switch_execution, base_url),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.journal",
        format!("cutover journal persisted at `{}`", journal_path.display()),
    )?;
    Ok(report)
}

fn planned_cutover_switch_report(
    evaluated: &EvaluatedImportCutover,
    base_url: &str,
    switch_execution: &CutoverSwitchExecution,
) -> Result<CommandReport, CliRunError> {
    let mut report = CommandReport::new(
        ["import", "cutover"],
        format!(
            "Planned cutover switch for import run `{}` against `{}`",
            evaluated.manifest.run_id, base_url
        ),
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to build cutover switch dry-run report for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    report.status = ReportStatus::Ok;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.switch.dry_run",
        format!(
            "{}; no provider state or cutover journal was modified",
            render_cutover_switch_detail(switch_execution, base_url)
        ),
    )?;
    for target in &switch_execution.traffic_targets {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "cutover.switch.target",
            format!(
                "{} `{}` would move from `{}` to `{}`",
                target.resource_kind,
                target.resource_id,
                target.previous_target,
                target.current_target
            ),
        )?;
    }
    for record in &switch_execution.dns_records {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "cutover.switch.dns",
            format!(
                "DNS hostname `{}` would move from `{}` to `{}`",
                record.hostname, record.previous_content, record.current_content
            ),
        )?;
    }
    Ok(report)
}

fn execute_cutover_switch(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CutoverSwitchExecution, CliRunError> {
    match evaluated.cutover.switch_method.as_deref() {
        Some("dns") => execute_dns_cutover_switch(invocation, evaluated),
        Some("load-balancer") => execute_load_balancer_cutover_switch(invocation, evaluated),
        Some("cdn-origin") => execute_cdn_origin_cutover_switch(invocation, evaluated),
        Some("routing") => execute_routing_cutover_switch(invocation, evaluated),
        Some(other) => Err(CliRunError::execution(format!(
            "cutover switch method `{other}` is declared but not yet executable"
        ))),
        None => Err(CliRunError::execution(
            "cutover manifest does not declare a switch method".to_string(),
        )),
    }
}

fn execute_dns_cutover_switch(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CutoverSwitchExecution, CliRunError> {
    validate_cutover_dns_hostnames(evaluated)?;
    validate_cutover_tls_readiness(evaluated)?;
    let request = resolve_dns_switch_request(invocation)?;
    let credentials = resolve_cloudflare_credentials(&evaluated.runtime.built)?;
    let client = build_cutover_provider_client("Cloudflare DNS switch")?;

    let mut execution = CutoverSwitchExecution::new("dns").map_err(import_model_error)?;
    for hostname in &evaluated.cutover.hostnames {
        let record =
            fetch_cloudflare_cname_record(&client, &credentials, &request.zone_id, hostname)?;
        let updated = if invocation.dry_run {
            CloudflareDnsRecord {
                id: record.id.clone(),
                name: hostname.clone(),
                record_type: record.record_type.clone(),
                content: request.target.clone(),
                proxied: record.proxied,
            }
        } else {
            update_cloudflare_cname_record(
                &client,
                &credentials,
                &request.zone_id,
                &record.id,
                hostname,
                &request.target,
                record.proxied,
            )?
        };
        execution = execution.with_dns_record(
            CutoverDnsRecordChange::new(
                hostname.clone(),
                request.zone_id.clone(),
                record.id.clone(),
                record.record_type,
                record.content,
                updated.content,
            )
            .map_err(import_model_error)?
            .with_previous_proxied(record.proxied)
            .with_current_proxied(updated.proxied),
        );
    }

    Ok(execution)
}

fn execute_load_balancer_cutover_switch(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CutoverSwitchExecution, CliRunError> {
    validate_cutover_dns_hostnames(evaluated)?;
    validate_cutover_tls_readiness(evaluated)?;
    let request = resolve_traffic_target_switch_request(invocation, "load-balancer")?;
    let credentials = resolve_cloudflare_credentials(&evaluated.runtime.built)?;
    let client = build_cutover_provider_client("Cloudflare load-balancer switch")?;
    let load_balancer = fetch_cloudflare_load_balancer(
        &client,
        &credentials,
        &request.zone_id,
        &request.resource_id,
    )?;
    let previous_target = load_balancer
        .default_pools
        .first()
        .cloned()
        .ok_or_else(|| {
            CliRunError::execution(format!(
                "Cloudflare load balancer `{}` does not declare a default pool target",
                request.resource_id
            ))
        })?;
    if !invocation.dry_run {
        update_cloudflare_load_balancer(
            &client,
            &credentials,
            &request.zone_id,
            &request.resource_id,
            &request.target,
        )?;
    }

    Ok(CutoverSwitchExecution::new("load-balancer")
        .map_err(import_model_error)?
        .with_traffic_target(
            CutoverTrafficTargetChange::new(
                "load_balancer",
                request.zone_id,
                request.resource_id,
                previous_target,
                request.target,
            )
            .map_err(import_model_error)?,
        ))
}

fn execute_cdn_origin_cutover_switch(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CutoverSwitchExecution, CliRunError> {
    validate_cutover_dns_hostnames(evaluated)?;
    validate_cutover_tls_readiness(evaluated)?;
    let request = resolve_traffic_target_switch_request(invocation, "cdn-origin")?;
    let credentials = resolve_cloudflare_credentials(&evaluated.runtime.built)?;
    let client = build_cutover_provider_client("Cloudflare CDN origin switch")?;
    let origin_rule = fetch_cloudflare_origin_rule(
        &client,
        &credentials,
        &request.zone_id,
        &request.resource_id,
    )?;
    if !invocation.dry_run {
        update_cloudflare_origin_rule(
            &client,
            &credentials,
            &request.zone_id,
            &request.resource_id,
            &request.target,
        )?;
    }

    Ok(CutoverSwitchExecution::new("cdn-origin")
        .map_err(import_model_error)?
        .with_traffic_target(
            CutoverTrafficTargetChange::new(
                "cdn_origin",
                request.zone_id,
                request.resource_id,
                origin_rule.origin,
                request.target,
            )
            .map_err(import_model_error)?,
        ))
}

fn execute_routing_cutover_switch(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CutoverSwitchExecution, CliRunError> {
    validate_cutover_dns_hostnames(evaluated)?;
    validate_cutover_tls_readiness(evaluated)?;
    let request = resolve_traffic_target_switch_request(invocation, "routing")?;
    let credentials = resolve_cloudflare_credentials(&evaluated.runtime.built)?;
    let client = build_cutover_provider_client("Cloudflare routing switch")?;
    let routing_rule = fetch_cloudflare_routing_rule(
        &client,
        &credentials,
        &request.zone_id,
        &request.resource_id,
    )?;
    if !invocation.dry_run {
        update_cloudflare_routing_rule(
            &client,
            &credentials,
            &request.zone_id,
            &request.resource_id,
            &request.target,
        )?;
    }

    Ok(CutoverSwitchExecution::new("routing")
        .map_err(import_model_error)?
        .with_traffic_target(
            CutoverTrafficTargetChange::new(
                "routing_rule",
                request.zone_id,
                request.resource_id,
                routing_rule.service,
                request.target,
            )
            .map_err(import_model_error)?,
        ))
}

fn render_cutover_switch_detail(execution: &CutoverSwitchExecution, base_url: &str) -> String {
    match execution.method.as_str() {
        "dns" => format!(
            "live DNS switch executed for {} hostname(s) and observation will target `{base_url}`",
            execution.dns_records.len()
        ),
        "load-balancer" => format!(
            "live load-balancer switch executed for {} target(s) and observation will target `{base_url}`",
            execution.traffic_targets.len()
        ),
        "cdn-origin" => format!(
            "live CDN origin switch executed for {} target(s) and observation will target `{base_url}`",
            execution.traffic_targets.len()
        ),
        "routing" => format!(
            "live routing switch executed for {} target(s) and observation will target `{base_url}`",
            execution.traffic_targets.len()
        ),
        _ => format!("live switch executed against `{base_url}`"),
    }
}

fn validate_cutover_dns_hostnames(evaluated: &EvaluatedImportCutover) -> Result<(), CliRunError> {
    if evaluated.cutover.hostnames.is_empty() {
        return Err(CliRunError::execution(
            "cutover switch requires at least one hostname".to_string(),
        ));
    }

    let declared = evaluated
        .runtime
        .built
        .manifest
        .domains
        .iter()
        .map(|domain| domain.hostname.as_str())
        .collect::<BTreeSet<_>>();
    let undeclared = evaluated
        .cutover
        .hostnames
        .iter()
        .filter(|hostname| !declared.contains(hostname.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !undeclared.is_empty() {
        return Err(CliRunError::execution(format!(
            "cutover hostnames must belong to the target customer app domains; undeclared hostnames: {}",
            undeclared.join(", ")
        )));
    }

    Ok(())
}

fn validate_cutover_tls_readiness(evaluated: &EvaluatedImportCutover) -> Result<(), CliRunError> {
    if evaluated.runtime.built.runtime_plan.runtime.config.tls.mode
        == davenda_config::TlsMode::External
    {
        return Ok(());
    }

    let host = evaluated
        .runtime
        .built
        .runtime_plan
        .runtime
        .tls_host()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to evaluate TLS readiness for cutover `{}`: {error}",
                evaluated.manifest.run_id
            ))
        })?;
    let snapshot = host.status();
    let ready = snapshot
        .inventory
        .certificates()
        .iter()
        .filter(|record| record.status == CertificateStatus::Active)
        .flat_map(|record| {
            record
                .bindings
                .iter()
                .map(|binding| binding.hostname.to_string())
        })
        .collect::<BTreeSet<_>>();
    let missing = evaluated
        .cutover
        .hostnames
        .iter()
        .filter(|hostname| !ready.contains(hostname.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(CliRunError::execution(format!(
            "cutover hostnames do not have active managed TLS coverage yet: {}",
            missing.join(", ")
        )));
    }

    Ok(())
}

fn resolve_dns_switch_request(
    invocation: &ImportCutoverInvocation,
) -> Result<CloudflareDnsSwitchRequest, CliRunError> {
    let zone_id = invocation
        .dns_zone_id
        .as_ref()
        .or(invocation.switch_zone_id.as_ref())
        .ok_or_else(|| {
        CliRunError::usage("`import cutover --switch` with `switch_method = \"dns\"` requires `--dns-zone-id <zone>`")
    })?;
    let target = invocation
        .dns_target
        .as_ref()
        .or(invocation.switch_target.as_ref())
        .ok_or_else(|| {
        CliRunError::usage("`import cutover --switch` with `switch_method = \"dns\"` requires `--dns-target <hostname>`")
    })?;
    if target.contains("://") || target.contains('/') {
        return Err(CliRunError::usage(
            "`--dns-target` must be a hostname, not a URL or path",
        ));
    }

    Ok(CloudflareDnsSwitchRequest {
        zone_id: zone_id.trim().to_string(),
        target: target.trim().trim_end_matches('.').to_string(),
    })
}

fn resolve_traffic_target_switch_request(
    invocation: &ImportCutoverInvocation,
    method: &str,
) -> Result<CloudflareTrafficTargetSwitchRequest, CliRunError> {
    let zone_id = invocation.switch_zone_id.as_ref().ok_or_else(|| {
        CliRunError::usage(format!(
            "`import cutover --switch` with `switch_method = \"{method}\"` requires `--switch-zone-id <zone>`"
        ))
    })?;
    let resource_id = invocation.switch_resource_id.as_ref().ok_or_else(|| {
        CliRunError::usage(format!(
            "`import cutover --switch` with `switch_method = \"{method}\"` requires `--switch-resource-id <id>`"
        ))
    })?;
    let target = invocation.switch_target.as_ref().ok_or_else(|| {
        CliRunError::usage(format!(
            "`import cutover --switch` with `switch_method = \"{method}\"` requires `--switch-target <target>`"
        ))
    })?;

    Ok(CloudflareTrafficTargetSwitchRequest {
        zone_id: zone_id.trim().to_string(),
        resource_id: resource_id.trim().to_string(),
        target: target.trim().to_string(),
    })
}

impl CloudflareCredentials {
    fn from_secret(secret: impl Into<String>) -> Self {
        let raw = secret.into();
        let payload = serde_json::from_str::<CloudflareSecretPayload>(&raw).unwrap_or_default();
        Self { raw, payload }
    }

    fn headers(&self) -> Result<HeaderMap, CliRunError> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        if let Some(token) = self.payload.cloudflare_api_token.as_deref().or_else(|| {
            (!self.raw.trim().is_empty() && !self.raw.trim_start().starts_with('{'))
                .then_some(self.raw.as_str())
        }) {
            let auth = format!("Bearer {token}");
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&auth).map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to build Cloudflare authorization header: {error}"
                    ))
                })?,
            );
            return Ok(headers);
        }

        if let Some(service_key) = self.payload.cloudflare_service_key.as_deref() {
            headers.insert(
                HeaderName::from_static("x-auth-user-service-key"),
                HeaderValue::from_str(service_key).map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to build Cloudflare service key header: {error}"
                    ))
                })?,
            );
            return Ok(headers);
        }

        if let (Some(email), Some(api_key)) = (
            self.payload.cloudflare_email.as_deref(),
            self.payload.cloudflare_api_key.as_deref(),
        ) {
            headers.insert(
                HeaderName::from_static("x-auth-email"),
                HeaderValue::from_str(email).map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to build Cloudflare email header: {error}"
                    ))
                })?,
            );
            headers.insert(
                HeaderName::from_static("x-auth-key"),
                HeaderValue::from_str(api_key).map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to build Cloudflare API key header: {error}"
                    ))
                })?,
            );
            return Ok(headers);
        }

        Err(CliRunError::execution(
            "Cloudflare DNS switch requires API credentials from `tls.account_secret` or `DAVENDA_CUTOVER_CLOUDFLARE_SECRET`".to_string(),
        ))
    }
}

fn resolve_cloudflare_credentials(
    built: &BuiltCustomerAppContext,
) -> Result<CloudflareCredentials, CliRunError> {
    if let Some(secret) = built
        .runtime_plan
        .runtime
        .config
        .tls
        .account_secret
        .as_ref()
    {
        if let Ok(value) = EnvironmentSecretResolver.resolve(secret) {
            return Ok(CloudflareCredentials::from_secret(value));
        }
    }

    let fallback = std::env::var(CUTOVER_CLOUDFLARE_SECRET_ENV).map_err(|_| {
        CliRunError::execution(format!(
            "Cloudflare DNS switch requires `tls.account_secret` or `{CUTOVER_CLOUDFLARE_SECRET_ENV}`"
        ))
    })?;
    Ok(CloudflareCredentials::from_secret(fallback))
}

fn build_cutover_provider_client(label: &str) -> Result<BlockingHttpClient, CliRunError> {
    BlockingHttpClient::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| {
            CliRunError::execution(format!("failed to build HTTP client for {label}: {error}"))
        })
}

fn cloudflare_api_base_url() -> String {
    std::env::var(CLOUDFLARE_API_BASE_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "https://api.cloudflare.com/client/v4".to_string())
}

fn fetch_cloudflare_cname_record(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    hostname: &str,
) -> Result<CloudflareDnsRecord, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/dns_records",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .get(url)
        .headers(credentials.headers()?)
        .query(&[("type", "CNAME"), ("name", hostname), ("per_page", "5")])
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to query Cloudflare DNS record for `{hostname}` in zone `{zone_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<Vec<CloudflareDnsRecord>>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare DNS lookup response for `{hostname}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare DNS lookup for `{hostname}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    let matches = envelope
        .result
        .into_iter()
        .filter(|record| record.name == hostname)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [record] => Ok(record.clone()),
        [] => Err(CliRunError::execution(format!(
            "Cloudflare DNS switch requires an existing CNAME record for `{hostname}` in zone `{zone_id}`"
        ))),
        _ => Err(CliRunError::execution(format!(
            "Cloudflare DNS switch found multiple CNAME records for `{hostname}` in zone `{zone_id}`"
        ))),
    }
}

fn update_cloudflare_cname_record(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    record_id: &str,
    hostname: &str,
    target: &str,
    proxied: Option<bool>,
) -> Result<CloudflareDnsRecord, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/dns_records/{record_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .put(url)
        .headers(credentials.headers()?)
        .json(&CloudflareDnsRecordUpdate {
            record_type: "CNAME",
            name: hostname,
            content: target,
            ttl: 1,
            proxied,
        })
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to update Cloudflare DNS record `{record_id}` for `{hostname}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<CloudflareDnsRecord>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare DNS update response for `{hostname}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare DNS update for `{hostname}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    Ok(envelope.result)
}

fn fetch_cloudflare_load_balancer(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    resource_id: &str,
) -> Result<CloudflareLoadBalancer, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/load_balancers/{resource_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .get(url)
        .headers(credentials.headers()?)
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to query Cloudflare load balancer `{resource_id}` in zone `{zone_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<CloudflareLoadBalancer>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare load balancer lookup for `{resource_id}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare load balancer lookup for `{resource_id}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    Ok(envelope.result)
}

fn update_cloudflare_load_balancer(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    resource_id: &str,
    target: &str,
) -> Result<CloudflareLoadBalancer, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/load_balancers/{resource_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .put(url)
        .headers(credentials.headers()?)
        .json(&CloudflareLoadBalancerUpdate {
            default_pools: vec![target],
        })
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to update Cloudflare load balancer `{resource_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<Value>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare load balancer update for `{resource_id}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare load balancer update for `{resource_id}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }

    let confirmed = fetch_cloudflare_load_balancer(client, credentials, zone_id, resource_id)?;
    if confirmed.default_pools.first().map(String::as_str) != Some(target) {
        return Err(CliRunError::execution(format!(
            "Cloudflare load balancer `{resource_id}` update acknowledged but now resolves to `{}` instead of `{target}`",
            confirmed
                .default_pools
                .first()
                .map(String::as_str)
                .unwrap_or("<none>")
        )));
    }
    Ok(confirmed)
}

fn fetch_cloudflare_origin_rule(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    resource_id: &str,
) -> Result<CloudflareOriginRule, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/origin_rules/{resource_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .get(url)
        .headers(credentials.headers()?)
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to query Cloudflare CDN origin rule `{resource_id}` in zone `{zone_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<CloudflareOriginRule>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare CDN origin rule lookup for `{resource_id}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare CDN origin lookup for `{resource_id}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    Ok(envelope.result)
}

fn update_cloudflare_origin_rule(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    resource_id: &str,
    target: &str,
) -> Result<CloudflareOriginRule, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/origin_rules/{resource_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .put(url)
        .headers(credentials.headers()?)
        .json(&CloudflareOriginRuleUpdate { origin: target })
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to update Cloudflare CDN origin rule `{resource_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<CloudflareOriginRule>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare CDN origin update for `{resource_id}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare CDN origin update for `{resource_id}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    Ok(envelope.result)
}

fn fetch_cloudflare_routing_rule(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    resource_id: &str,
) -> Result<CloudflareRoutingRule, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/routing_rules/{resource_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .get(url)
        .headers(credentials.headers()?)
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to query Cloudflare routing rule `{resource_id}` in zone `{zone_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<CloudflareRoutingRule>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare routing rule lookup for `{resource_id}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare routing rule lookup for `{resource_id}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    Ok(envelope.result)
}

fn update_cloudflare_routing_rule(
    client: &BlockingHttpClient,
    credentials: &CloudflareCredentials,
    zone_id: &str,
    resource_id: &str,
    target: &str,
) -> Result<CloudflareRoutingRule, CliRunError> {
    let url = format!(
        "{}/zones/{zone_id}/routing_rules/{resource_id}",
        cloudflare_api_base_url().trim_end_matches('/')
    );
    let response = client
        .put(url)
        .headers(credentials.headers()?)
        .json(&CloudflareRoutingRuleUpdate { service: target })
        .send()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to update Cloudflare routing rule `{resource_id}`: {error}"
            ))
        })?;
    let envelope = response
        .json::<CloudflareResponseEnvelope<CloudflareRoutingRule>>()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse Cloudflare routing rule update for `{resource_id}`: {error}"
            ))
        })?;
    if !envelope.success {
        return Err(CliRunError::execution(format!(
            "Cloudflare routing rule update for `{resource_id}` failed: {}",
            render_cloudflare_errors(&envelope.errors)
        )));
    }
    Ok(envelope.result)
}

fn render_cloudflare_errors(errors: &[CloudflareError]) -> String {
    if errors.is_empty() {
        "unknown Cloudflare error".to_string()
    } else {
        errors
            .iter()
            .map(|error| match error.code {
                Some(code) => format!("{code}: {}", error.message),
                None => error.message.clone(),
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedCutoverRoute {
    route: String,
    status_code: u16,
    outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CutoverObservationProbe {
    health_status: String,
    readiness_status: String,
    maintenance_enabled: bool,
    routes: Vec<ObservedCutoverRoute>,
    failures: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ObservationVerificationChecks {
    record_counts: bool,
    cache_leaks: bool,
    route_resolution: bool,
    canonical_urls: bool,
    media_reachability: bool,
    fragment_rendering: bool,
    session_creation: bool,
    auth_failures: bool,
    transactional_journey_errors: bool,
    webhook_failures: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedLiveRoutePayload {
    status_code: u16,
    headers: BTreeMap<String, String>,
    body: String,
}

fn observe_import_cutover(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CommandReport, CliRunError> {
    if !invocation.confirmed {
        return Err(CliRunError::usage(
            "`import cutover --observe` requires `--yes`",
        ));
    }
    let base_url = invocation.base_url.as_ref().ok_or_else(|| {
        CliRunError::usage("`import cutover --observe` requires `--base-url <url>`")
    })?;
    let observation_window_minutes = evaluated
        .cutover
        .observation_window_minutes
        .unwrap_or_default();
    let sample_routes = evaluated
        .manifest
        .verification
        .as_ref()
        .map(|verification| verification.sample_routes.clone())
        .unwrap_or_default();
    let verification_checks = evaluated
        .manifest
        .verification
        .as_ref()
        .map(|verification| {
            build_cutover_verification_checks(verification, &evaluated.verification_support)
        })
        .transpose()?
        .unwrap_or_default();
    if sample_routes.is_empty() && cutover_observation_requires_sample_routes(verification_checks) {
        return Err(CliRunError::execution(
            "cutover observation requires `[verification].sample_routes` so live public routes can be probed"
                .to_string(),
        ));
    }

    let journal_path = cutover_journal_path(&invocation.manifest_path, &evaluated.manifest.run_id);
    let expected_steps = cutover_steps(&evaluated.cutover)?;
    let mut journal = CutoverExecutionJournal::load(
        &journal_path,
        &evaluated.manifest.run_id,
        evaluated.manifest.customer_app_id.as_str(),
        expected_steps,
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to load cutover journal for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;

    match journal.state {
        davenda_import::CutoverExecutionState::SwitchConfirmed
        | davenda_import::CutoverExecutionState::Observing
        | davenda_import::CutoverExecutionState::ObservationPassed
        | davenda_import::CutoverExecutionState::RollbackRequired => {}
        _ => {
            return Err(CliRunError::execution(format!(
                "cutover `{}` must be switched with `platform import cutover {} --switch --base-url <url> --yes` before observation can start",
                evaluated.manifest.run_id,
                invocation.manifest_path.display()
            )));
        }
    }

    let probe_time = unix_timestamp_now()?;
    journal.begin_observation(base_url.clone(), probe_time);
    let observation_started_at = journal
        .observation_started_at_unix_seconds
        .unwrap_or(probe_time);
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;

    let client = build_cutover_probe_client()?;
    let probe = execute_cutover_observation_probe(
        &evaluated.runtime.built,
        &client,
        base_url,
        evaluated.manifest.verification.as_ref(),
        &sample_routes,
        evaluated.verification_support.auth_probe.as_ref(),
        &evaluated.verification_support.transactional_probes,
        verification_checks,
        observation_started_at,
    )?;

    if !probe.failures.is_empty() {
        journal
            .mark_step_failed(
                "cutover.observe",
                format!(
                    "live observation failed against `{base_url}`: {}",
                    probe.failures.join("; ")
                ),
            )
            .map_err(import_model_error)?;
        journal.mark_rollback_required(base_url.clone(), probe_time, probe.failures.clone());
        save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;
        return Err(CliRunError::execution(format!(
            "cutover `{}` requires rollback review: {}",
            evaluated.manifest.run_id,
            probe.failures.join("; ")
        )));
    }

    let elapsed_seconds = probe_time.saturating_sub(observation_started_at);
    let required_seconds = observation_window_minutes.saturating_mul(60) as u64;
    let window_elapsed = elapsed_seconds >= required_seconds;
    if window_elapsed {
        journal.mark_observation_passed(probe_time);
        journal
            .mark_step_completed(
                "cutover.observe",
                format!(
                    "live observation stayed green for {} minute(s) against `{base_url}`",
                    observation_window_minutes
                ),
            )
            .map_err(import_model_error)?;
    } else {
        journal.begin_observation(base_url.clone(), probe_time);
    }
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;

    let mut report = journal.command_report().map_err(|error| {
        CliRunError::execution(format!(
            "failed to render cutover observation report for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    report.summary = if window_elapsed {
        format!(
            "Cutover observation for import run `{}` passed against `{}`",
            evaluated.manifest.run_id, base_url
        )
    } else {
        format!(
            "Cutover observation for import run `{}` remains in progress with {} second(s) remaining against `{}`",
            evaluated.manifest.run_id,
            required_seconds.saturating_sub(elapsed_seconds),
            base_url
        )
    };
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.observe.base_url",
        format!("observing live traffic through `{base_url}`"),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.observe.health",
        format!(
            "health=`{}`, readiness=`{}`, maintenance_enabled={}",
            probe.health_status, probe.readiness_status, probe.maintenance_enabled
        ),
    )?;
    for route in &probe.routes {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "cutover.observe.route",
            format!(
                "route `{}` returned {} ({})",
                route.route, route.status_code, route.outcome
            ),
        )?;
    }
    if !window_elapsed {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "cutover.observe.window",
            format!(
                "observation window started at {} and still has {} second(s) remaining",
                observation_started_at,
                required_seconds.saturating_sub(elapsed_seconds)
            ),
        )?;
    }
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.journal",
        format!("cutover journal persisted at `{}`", journal_path.display()),
    )?;
    Ok(report)
}

fn rollback_import_cutover(
    invocation: &ImportCutoverInvocation,
    evaluated: &EvaluatedImportCutover,
) -> Result<CommandReport, CliRunError> {
    if invocation.switch_plan_path.is_some() {
        return Err(CliRunError::usage(
            "`import cutover --rollback` does not yet consume `--switch-plan`; rollback reuses the provider-managed switch metadata recorded in the cutover journal",
        ));
    }
    if !invocation.confirmed {
        return Err(CliRunError::usage(
            "`import cutover --rollback` requires `--yes`",
        ));
    }
    let base_url = invocation.base_url.as_ref().ok_or_else(|| {
        CliRunError::usage("`import cutover --rollback` requires `--base-url <url>`")
    })?;
    let reason = invocation.reason.as_ref().ok_or_else(|| {
        CliRunError::usage("`import cutover --rollback` requires `--reason <text>`")
    })?;

    let journal_path = cutover_journal_path(&invocation.manifest_path, &evaluated.manifest.run_id);
    let expected_steps = cutover_steps(&evaluated.cutover)?;
    let mut journal = CutoverExecutionJournal::load(
        &journal_path,
        &evaluated.manifest.run_id,
        evaluated.manifest.customer_app_id.as_str(),
        expected_steps,
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to load cutover journal for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;

    match journal.state {
        davenda_import::CutoverExecutionState::SwitchConfirmed
        | davenda_import::CutoverExecutionState::Observing
        | davenda_import::CutoverExecutionState::ObservationPassed
        | davenda_import::CutoverExecutionState::RollbackRequired
        | davenda_import::CutoverExecutionState::RolledBack => {}
        _ => {
            return Err(CliRunError::execution(format!(
                "cutover `{}` cannot be rolled back before the live switch has been confirmed",
                evaluated.manifest.run_id
            )));
        }
    }

    let rollback_detail = restore_cutover_switch(&journal, &evaluated.runtime.built)?;
    let rolled_back_at = unix_timestamp_now()?;
    run_cutover_step(
        &mut journal,
        &journal_path,
        &evaluated.manifest.run_id,
        "rollback.executed",
        || Ok(format!("{rollback_detail}; rollback reason: {reason}")),
    )?;
    journal
        .mark_rolled_back(base_url.clone(), rolled_back_at, reason.clone())
        .map_err(import_model_error)?;
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;

    let mut report = journal.command_report().map_err(|error| {
        CliRunError::execution(format!(
            "failed to render cutover rollback report for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    report.summary = format!(
        "Cutover rollback for import run `{}` executed against `{}`",
        evaluated.manifest.run_id, base_url
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Warning,
        "cutover.rollback",
        format!("{rollback_detail}; rollback confirmed for `{base_url}`: {reason}"),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.journal",
        format!("cutover journal persisted at `{}`", journal_path.display()),
    )?;
    Ok(report)
}

fn restore_cutover_switch(
    journal: &CutoverExecutionJournal,
    built: &BuiltCustomerAppContext,
) -> Result<String, CliRunError> {
    let Some(execution) = journal.switch_execution.as_ref() else {
        return Ok(
            "no provider-managed switch state was recorded; rollback remained operator-owned"
                .to_string(),
        );
    };

    match execution.method.as_str() {
        "dns" => {
            let credentials = resolve_cloudflare_credentials(built)?;
            let client = build_cutover_provider_client("Cloudflare DNS rollback")?;
            for record in &execution.dns_records {
                update_cloudflare_cname_record(
                    &client,
                    &credentials,
                    &record.zone_id,
                    &record.record_id,
                    &record.hostname,
                    &record.previous_content,
                    record.previous_proxied,
                )?;
            }
            Ok(format!(
                "restored {} DNS hostname(s) to their pre-cutover targets",
                execution.dns_records.len()
            ))
        }
        "load-balancer" => {
            let credentials = resolve_cloudflare_credentials(built)?;
            let client = build_cutover_provider_client("Cloudflare load-balancer rollback")?;
            for target in &execution.traffic_targets {
                update_cloudflare_load_balancer(
                    &client,
                    &credentials,
                    &target.zone_id,
                    &target.resource_id,
                    &target.previous_target,
                )?;
            }
            Ok(format!(
                "restored {} load-balancer target(s) to their pre-cutover state",
                execution.traffic_targets.len()
            ))
        }
        "cdn-origin" => {
            let credentials = resolve_cloudflare_credentials(built)?;
            let client = build_cutover_provider_client("Cloudflare CDN origin rollback")?;
            for target in &execution.traffic_targets {
                update_cloudflare_origin_rule(
                    &client,
                    &credentials,
                    &target.zone_id,
                    &target.resource_id,
                    &target.previous_target,
                )?;
            }
            Ok(format!(
                "restored {} CDN origin target(s) to their pre-cutover state",
                execution.traffic_targets.len()
            ))
        }
        "routing" => {
            let credentials = resolve_cloudflare_credentials(built)?;
            let client = build_cutover_provider_client("Cloudflare routing rollback")?;
            for target in &execution.traffic_targets {
                update_cloudflare_routing_rule(
                    &client,
                    &credentials,
                    &target.zone_id,
                    &target.resource_id,
                    &target.previous_target,
                )?;
            }
            Ok(format!(
                "restored {} routing target(s) to their pre-cutover state",
                execution.traffic_targets.len()
            ))
        }
        other => Err(CliRunError::execution(format!(
            "cutover rollback does not yet know how to restore switch method `{other}`"
        ))),
    }
}

fn evaluate_import_cutover(
    invocation: &ImportCutoverInvocation,
) -> Result<EvaluatedImportCutover, CliRunError> {
    let manifest = ImportManifest::from_file(&invocation.manifest_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load import manifest from `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let manifest_root = invocation
        .manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    manifest.validate_at(manifest_root).map_err(|error| {
        CliRunError::execution(format!(
            "failed to validate import manifest `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let plan = manifest.plan().map_err(|error| {
        CliRunError::execution(format!(
            "failed to plan import manifest `{}`: {error}",
            invocation.manifest_path.display()
        ))
    })?;
    let cutover = manifest.cutover.clone().ok_or_else(|| {
        CliRunError::execution(format!(
            "import manifest `{}` does not declare a `[cutover]` section",
            invocation.manifest_path.display()
        ))
    })?;
    let runtime = build_import_runtime_context(manifest_root, &manifest)?.ok_or_else(|| {
        CliRunError::execution(format!(
            "import manifest `{}` does not declare a target runtime",
            invocation.manifest_path.display()
        ))
    })?;
    let verification_support = build_cutover_verification_support(&runtime.built);

    let config_path = manifest_root.join(
        manifest
            .target
            .as_ref()
            .expect("validated cutover manifests always declare a target")
            .platform_config
            .as_str(),
    );
    let mut cutover_plan = CutoverPlan::new()
        .with_check(build_cutover_check(
            "import.package",
            "import package references, target alignment, and cutover metadata validated",
            true,
            true,
        )?)
        .with_check(build_cutover_check(
            "target.runtime",
            format!(
                "target runtime `{}` resolves the declared customer app and modules",
                runtime.built.manifest.id
            ),
            true,
            true,
        )?)
        .with_check(build_cutover_check(
            "final.import.mode",
            format!(
                "import publication mode `{}` can materialize live runtime state",
                publication_mode_label(plan.publication_mode)
            ),
            true,
            plan.publication_mode == PublicationMode::PublishValidated,
        )?);

    if let Some(verification) = &manifest.verification {
        let (verification_ready, verification_detail) =
            evaluate_verification_readiness(verification, &runtime.built, &verification_support);
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "verification.plan",
            verification_detail,
            true,
            verification_ready,
        )?);
    }

    let release_report = runtime
        .built
        .runtime_plan
        .release_doctor
        .command_report()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to render release doctor report for cutover `{}`: {error}",
                invocation.manifest_path.display()
            ))
        })?;
    cutover_plan = cutover_plan.with_check(build_cutover_check(
        "release.doctor",
        "release doctor must not report blocking findings before traffic moves",
        true,
        release_report.status != ReportStatus::Unsafe,
    )?);

    if cutover.requires_storage_validation {
        let storage_report = run_storage_verify(&config_path, true)?;
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "storage.verify",
            "storage policy and backend validation must be green",
            true,
            storage_report.status == ReportStatus::Ok,
        )?);
    }

    if cutover.requires_assets_publish {
        let assets_report = run_assets_publish(
            &AssetsPublishInvocation {
                config_path: config_path.clone(),
                confirmed: false,
            },
            true,
        )?;
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "assets.publish",
            "theme asset publication must plan cleanly against the target runtime",
            true,
            assets_report.status == ReportStatus::Ok,
        )?);
    }

    if cutover.requires_migrate_apply {
        let (migrations_ready, migrations_detail) =
            evaluate_cutover_migration_readiness(&config_path, &runtime.built)?;
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "migrate.apply",
            migrations_detail,
            true,
            migrations_ready,
        )?);
    }

    let (auth_ready, auth_detail) = evaluate_auth_package_validation_readiness(&config_path);
    cutover_plan = cutover_plan.with_check(build_cutover_check(
        "auth.package.validate",
        if auth_ready {
            format!(
                "auth package `{}` capability bindings validate against the target customer app",
                runtime.built.manifest.auth.package_name
            )
        } else {
            format!(
                "auth package `{}` validation must succeed before traffic moves: {}",
                runtime.built.manifest.auth.package_name, auth_detail
            )
        },
        true,
        auth_ready,
    )?);

    if cutover.requires_cache_warm {
        let (cache_ready, cache_detail) = match manifest.verification.as_ref() {
            Some(verification) if !verification.sample_routes.is_empty() => {
                match warm_cache_routes(
                    &runtime.built,
                    &verification.sample_routes,
                    "public",
                    true,
                ) {
                    Ok(_) => (
                        true,
                        format!(
                            "cache warm plan validated for routes: {}",
                            verification.sample_routes.join(", ")
                        ),
                    ),
                    Err(error) => (false, error.to_string()),
                }
            }
            Some(_) => (
                false,
                "cache warm requires verification.sample_routes so representative public routes can be warmed"
                    .to_string(),
            ),
            None => (
                false,
                "cache warm requires a `[verification]` section with sample routes".to_string(),
            ),
        };
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "cache.warm",
            cache_detail,
            true,
            cache_ready,
        )?);
    }

    for trigger in &cutover.rollback_triggers {
        cutover_plan = cutover_plan.with_trigger(
            RollbackTrigger::new(trigger.id.clone(), trigger.description.clone())
                .map_err(|error| CliRunError::execution(error.to_string()))?,
        );
    }

    let mut report = cutover_plan.command_report().map_err(|error| {
        CliRunError::execution(format!("failed to render cutover plan: {error}"))
    })?;
    report.command = vec!["import".to_string(), "cutover".to_string()];
    report.summary = format!(
        "Cutover readiness for import run `{}` into customer app `{}` via `{}`",
        manifest.run_id,
        runtime.built.manifest.id,
        cutover.switch_method.as_deref().unwrap_or("unknown")
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.hostnames",
        format!("cutover hostnames: {}", cutover.hostnames.join(", ")),
    )?;
    if cutover.freeze_legacy_writes {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "cutover.freeze",
            "legacy writes must be frozen before the final import and switch",
        )?;
    }
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.observation_window",
        format!(
            "observation window: {} minutes",
            cutover.observation_window_minutes.unwrap_or_default()
        ),
    )?;

    Ok(EvaluatedImportCutover {
        manifest,
        cutover,
        runtime,
        config_path,
        cutover_plan,
        verification_support,
        report,
    })
}

fn cutover_preflight_ready(plan: &CutoverPlan) -> bool {
    plan.checks.iter().all(|check| {
        if !check.required {
            return true;
        }
        match check.id.as_str() {
            "import.package"
            | "target.runtime"
            | "final.import.mode"
            | "release.doctor"
            | "auth.package.validate" => check.satisfied,
            _ => true,
        }
    })
}

fn cutover_steps(
    cutover: &davenda_import::ImportCutover,
) -> Result<Vec<CutoverStepRecord>, CliRunError> {
    let mut steps = vec![
        CutoverStepRecord::new(
            "final.import",
            "Final publish-validated import executed against the target runtime",
        )
        .map_err(import_model_error)?,
    ];
    if cutover.requires_storage_validation {
        steps.push(
            CutoverStepRecord::new(
                "storage.verify",
                "Storage policy and backend validation completed against the target runtime",
            )
            .map_err(import_model_error)?,
        );
    }
    if cutover.requires_assets_publish {
        steps.push(
            CutoverStepRecord::new(
                "assets.publish",
                "Theme asset publication completed against the target runtime",
            )
            .map_err(import_model_error)?,
        );
    }
    if cutover.requires_migrate_apply {
        steps.push(
            CutoverStepRecord::new(
                "migrate.apply",
                "Pending executable migrations were applied and auth package validation completed against the target runtime",
            )
            .map_err(import_model_error)?,
        );
    }
    if cutover.requires_cache_warm {
        steps.push(
            CutoverStepRecord::new(
                "cache.warm",
                "Representative public routes were warmed through the live runtime",
            )
            .map_err(import_model_error)?,
        );
    }
    steps.push(
        CutoverStepRecord::new(
            "cutover.readiness",
            "Readiness was re-evaluated after preparation and returned green",
        )
        .map_err(import_model_error)?,
    );
    steps.push(
        CutoverStepRecord::new(
            "switch.confirmed",
            "The operator confirmed that live routing or edge traffic now targets the new platform",
        )
        .map_err(import_model_error)?,
    );
    steps.push(
        CutoverStepRecord::new(
            "cutover.observe",
            "The live system remained healthy across the declared post-switch observation window",
        )
        .map_err(import_model_error)?,
    );
    steps.push(
        CutoverStepRecord::new(
            "rollback.executed",
            "The operator recorded a rollback after the live switch and documented the reason",
        )
        .map_err(import_model_error)?,
    );
    Ok(steps)
}

fn run_cutover_step<F>(
    journal: &mut CutoverExecutionJournal,
    journal_path: &Path,
    run_id: &impl std::fmt::Display,
    step_id: &str,
    step: F,
) -> Result<(), CliRunError>
where
    F: FnOnce() -> Result<String, CliRunError>,
{
    if journal.step_completed(step_id) {
        return Ok(());
    }

    match step() {
        Ok(detail) => {
            journal
                .mark_step_completed(step_id, detail)
                .map_err(import_model_error)?;
            save_cutover_journal(journal, journal_path, run_id)
        }
        Err(error) => {
            journal
                .mark_step_failed(step_id, error.to_string())
                .map_err(import_model_error)?;
            save_cutover_journal(journal, journal_path, run_id)?;
            Err(error)
        }
    }
}

fn save_cutover_journal(
    journal: &CutoverExecutionJournal,
    journal_path: &Path,
    run_id: &impl std::fmt::Display,
) -> Result<(), CliRunError> {
    journal.save(journal_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to persist cutover journal for `{run_id}`: {error}"
        ))
    })
}

fn build_cutover_probe_client() -> Result<BlockingHttpClient, CliRunError> {
    BlockingHttpClient::builder()
        .timeout(Duration::from_secs(5))
        .redirect(RedirectPolicy::limited(5))
        .build()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build HTTP client for cutover observation: {error}"
            ))
        })
}

fn build_cutover_verification_support(
    built: &BuiltCustomerAppContext,
) -> CutoverVerificationSupport {
    let fragment_probe = built
        .runtime_plan
        .runtime
        .http
        .routes
        .iter()
        .find(|route| route.area == RouteArea::Fragment && !route.path.contains('{'))
        .map(|route| VerificationRouteProbe {
            name: route.name.clone(),
            method: route.method,
            path: route.path.clone(),
            auth: route.auth,
        });
    let auth_probe = built
        .runtime_plan
        .runtime
        .http
        .routes
        .iter()
        .find(|route| !matches!(route.auth, RouteAuthGate::Public) && !route.path.contains('{'))
        .map(|route| VerificationRouteProbe {
            name: route.name.clone(),
            method: route.method,
            path: route.path.clone(),
            auth: route.auth,
        });
    let transactional_probes = built
        .runtime_plan
        .runtime
        .http
        .routes
        .iter()
        .filter(|route| {
            route.method.is_state_changing()
                && matches!(
                    route.area,
                    RouteArea::Public | RouteArea::Account | RouteArea::Api
                )
        })
        .map(|route| VerificationRouteProbe {
            name: route.name.clone(),
            method: route.method,
            path: route.path.clone(),
            auth: route.auth,
        })
        .collect();
    let webhook_probes = built
        .runtime_plan
        .runtime
        .extension_registry
        .registered_handlers()
        .iter()
        .filter(|handler| handler.point.to_string() == "webhook")
        .map(|handler| VerificationWebhookProbe {
            extension_id: handler.extension_id.to_string(),
            handler_id: handler.handler_id.to_string(),
            source: handler.surface.clone(),
            event: handler
                .selector
                .split_once('/')
                .map(|(_, event)| event.to_string())
                .unwrap_or_else(|| handler.selector.clone()),
        })
        .collect();

    CutoverVerificationSupport {
        fragment_probe,
        auth_probe,
        transactional_probes,
        webhook_probes,
    }
}

fn evaluate_verification_readiness(
    verification: &davenda_import::ImportVerification,
    built: &BuiltCustomerAppContext,
    support: &CutoverVerificationSupport,
) -> (bool, String) {
    match build_cutover_verification_checks(verification, support).and_then(|checks| {
        let detail =
            execute_local_cutover_verification_checks(verification, built, support, checks)?;
        Ok(format!(
            "verification checks supported: {}; {}",
            render_supported_verification_checks(verification, checks),
            detail
        ))
    }) {
        Ok(detail) => (true, detail),
        Err(error) => (false, error.to_string()),
    }
}

fn build_cutover_verification_checks(
    verification: &davenda_import::ImportVerification,
    support: &CutoverVerificationSupport,
) -> Result<ObservationVerificationChecks, CliRunError> {
    let mut checks = ObservationVerificationChecks::default();
    for required in &verification.required {
        match required.as_str() {
            "record_counts" => checks.record_counts = true,
            "cache_leak" | "cache_leaks" => checks.cache_leaks = true,
            "route_resolution" => checks.route_resolution = true,
            "canonical_urls" => checks.canonical_urls = true,
            "media_reachability" => checks.media_reachability = true,
            "fragment_rendering" => checks.fragment_rendering = true,
            "session_creation" => checks.session_creation = true,
            "auth_failure" | "auth_failures" => checks.auth_failures = true,
            "transactional_journey_errors" => checks.transactional_journey_errors = true,
            "webhook_failure" | "webhook_failures" => checks.webhook_failures = true,
            other => {
                return Err(CliRunError::execution(format!(
                    "verification check `{other}` is not yet supported by cutover verification"
                )));
            }
        }
    }

    if (checks.route_resolution || checks.canonical_urls || checks.media_reachability)
        && verification.sample_routes.is_empty()
    {
        return Err(CliRunError::execution(
            "verification checks that require live route probes must declare `[verification].sample_routes`"
                .to_string(),
        ));
    }
    if checks.cache_leaks && verification.sample_routes.is_empty() {
        return Err(CliRunError::execution(
            "verification check `cache_leaks` requires `[verification].sample_routes`".to_string(),
        ));
    }
    if checks.cache_leaks && verification.sample_users.is_empty() {
        return Err(CliRunError::execution(
            "verification check `cache_leaks` requires `[verification].sample_users`".to_string(),
        ));
    }
    if checks.fragment_rendering && support.fragment_probe.is_none() {
        return Err(CliRunError::execution(
            "verification check `fragment_rendering` requires at least one fragment route in the target runtime"
                .to_string(),
        ));
    }
    if checks.session_creation && verification.sample_users.is_empty() {
        return Err(CliRunError::execution(
            "verification check `session_creation` requires `[verification].sample_users`"
                .to_string(),
        ));
    }
    if checks.auth_failures && support.auth_probe.is_none() {
        return Err(CliRunError::execution(
            "verification check `auth_failures` requires at least one session- or capability-gated route in the target runtime"
                .to_string(),
        ));
    }
    if checks.transactional_journey_errors && support.transactional_probes.is_empty() {
        return Err(CliRunError::execution(
            "verification check `transactional_journey_errors` requires at least one public or account state-changing route in the target runtime"
                .to_string(),
        ));
    }
    if checks.webhook_failures && verification.webhooks.is_empty() {
        return Err(CliRunError::execution(
            "verification check `webhook_failures` requires `[[verification.webhooks]]` declarations"
                .to_string(),
        ));
    }
    if checks.webhook_failures {
        let missing = missing_webhook_verification_probes(verification, support);
        if !missing.is_empty() {
            return Err(CliRunError::execution(format!(
                "verification check `webhook_failures` requires installed webhook handlers for: {}",
                missing.join(", ")
            )));
        }
    }

    Ok(checks)
}

fn cutover_observation_requires_sample_routes(checks: ObservationVerificationChecks) -> bool {
    checks.cache_leaks
        || checks.route_resolution
        || checks.canonical_urls
        || checks.media_reachability
        || checks.transactional_journey_errors
}

fn render_supported_verification_checks(
    verification: &davenda_import::ImportVerification,
    checks: ObservationVerificationChecks,
) -> String {
    let mut rendered = Vec::new();
    if checks.record_counts
        || verification
            .required
            .iter()
            .any(|check| check == "record_counts")
    {
        rendered.push("record_counts(import-run)");
    }
    if checks.cache_leaks {
        rendered.push("cache_leaks(local+observe)");
    }
    if checks.fragment_rendering {
        rendered.push("fragment_rendering(local)");
    }
    if checks.session_creation {
        rendered.push("session_creation(local)");
    }
    if checks.auth_failures {
        rendered.push("auth_failures(local+observe)");
    }
    if checks.transactional_journey_errors {
        rendered.push("transactional_journey_errors(local+observe)");
    }
    if checks.route_resolution {
        rendered.push("route_resolution(observe)");
    }
    if checks.canonical_urls {
        rendered.push("canonical_urls(observe)");
    }
    if checks.media_reachability {
        rendered.push("media_reachability(observe)");
    }
    if checks.webhook_failures {
        rendered.push("webhook_failures(local+observe)");
    }
    rendered.join(", ")
}

fn execute_local_cutover_verification_checks(
    verification: &davenda_import::ImportVerification,
    built: &BuiltCustomerAppContext,
    support: &CutoverVerificationSupport,
    checks: ObservationVerificationChecks,
) -> Result<String, CliRunError> {
    let mut completed = Vec::new();

    if checks.session_creation {
        let sample_user = verification
            .sample_users
            .first()
            .ok_or_else(|| {
                CliRunError::execution(
                    "verification check `session_creation` requires `[verification].sample_users`"
                        .to_string(),
                )
            })?
            .clone();
        verify_session_creation_probe(built, &sample_user)?;
        completed.push(format!("session_creation(user:{sample_user})"));
    }

    if checks.fragment_rendering {
        let route = support.fragment_probe.as_ref().ok_or_else(|| {
            CliRunError::execution(
                "verification check `fragment_rendering` requires a fragment probe route"
                    .to_string(),
            )
        })?;
        verify_fragment_rendering_probe(built, route, verification.sample_users.first())?;
        completed.push(format!("fragment_rendering({})", route.path));
    }

    if checks.auth_failures {
        let route = support.auth_probe.as_ref().ok_or_else(|| {
            CliRunError::execution(
                "verification check `auth_failures` requires an auth probe route".to_string(),
            )
        })?;
        verify_local_auth_failure_probe(built, route)?;
        completed.push(format!("auth_failures({})", route.path));
    }
    if checks.transactional_journey_errors {
        let routes = derive_transactional_probe_routes(verification, support)?;
        for route in &routes {
            verify_local_transactional_probe(built, route)?;
            completed.push(format!(
                "transactional_journey_errors({} {})",
                render_http_method(route.method),
                route.path
            ));
        }
    }
    if checks.cache_leaks {
        let sample_user = verification.sample_users.first().ok_or_else(|| {
            CliRunError::execution(
                "verification check `cache_leaks` requires `[verification].sample_users`"
                    .to_string(),
            )
        })?;
        completed.push(format!(
            "cache_leaks(routes:{} user:{sample_user})",
            verification.sample_routes.len()
        ));
    }
    if checks.webhook_failures {
        for probe in verify_local_webhook_failure_probes(verification, support)? {
            completed.push(format!("webhook_failures({probe})"));
        }
    }

    if completed.is_empty() {
        completed.push("no local verification probes required".to_string());
    }

    Ok(format!(
        "local verification probes passed: {}",
        completed.join(", ")
    ))
}

fn verify_local_webhook_failure_probes(
    verification: &davenda_import::ImportVerification,
    support: &CutoverVerificationSupport,
) -> Result<Vec<String>, CliRunError> {
    let missing = missing_webhook_verification_probes(verification, support);
    if !missing.is_empty() {
        return Err(CliRunError::execution(format!(
            "verification check `webhook_failures` requires installed webhook handlers for: {}",
            missing.join(", ")
        )));
    }

    Ok(verification
        .webhooks
        .iter()
        .filter_map(|webhook| {
            support
                .webhook_probes
                .iter()
                .find(|probe| probe.source == webhook.source && probe.event == webhook.event)
                .map(|probe| {
                    format!(
                        "{}/{} via {}:{}",
                        probe.source, probe.event, probe.extension_id, probe.handler_id
                    )
                })
        })
        .collect())
}

fn missing_webhook_verification_probes(
    verification: &davenda_import::ImportVerification,
    support: &CutoverVerificationSupport,
) -> Vec<String> {
    verification
        .webhooks
        .iter()
        .filter(|webhook| {
            !support
                .webhook_probes
                .iter()
                .any(|probe| probe.source == webhook.source && probe.event == webhook.event)
        })
        .map(|webhook| format!("{}/{}", webhook.source, webhook.event))
        .collect()
}

fn derive_transactional_probe_routes(
    verification: &davenda_import::ImportVerification,
    support: &CutoverVerificationSupport,
) -> Result<Vec<VerificationRouteProbe>, CliRunError> {
    let mut derived = Vec::new();
    for route in &support.transactional_probes {
        if !route.path.contains('{') {
            derived.push(route.clone());
            continue;
        }
        for sample_route in &verification.sample_routes {
            if let Some(path) =
                derive_transactional_probe_path(route.path.as_str(), sample_route.as_str())
            {
                derived.push(VerificationRouteProbe {
                    name: route.name.clone(),
                    method: route.method,
                    path,
                    auth: route.auth,
                });
                break;
            }
        }
    }

    if derived.is_empty() {
        return Err(CliRunError::execution(
            "verification check `transactional_journey_errors` requires `[verification].sample_routes` that resolve at least one concrete transactional route"
                .to_string(),
        ));
    }

    Ok(derived)
}

fn derive_transactional_probe_path(pattern: &str, sample_route: &str) -> Option<String> {
    let pattern_segments = pattern
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let sample_segments = sample_route
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if pattern_segments.is_empty() {
        return (sample_route == "/").then_some("/".to_string());
    }

    if sample_segments.len() > pattern_segments.len() {
        return None;
    }

    let mut rendered = Vec::with_capacity(pattern_segments.len());
    for (index, segment) in pattern_segments.iter().enumerate() {
        if index < sample_segments.len() {
            if segment.starts_with('{') && segment.ends_with('}') {
                rendered.push(sample_segments[index].to_string());
            } else if *segment == sample_segments[index] {
                rendered.push(segment.to_string());
            } else {
                return None;
            }
        } else if segment.starts_with('{') && segment.ends_with('}') {
            return None;
        } else {
            rendered.push(segment.to_string());
        }
    }

    Some(format!("/{}", rendered.join("/")))
}

fn verify_local_transactional_probe(
    built: &BuiltCustomerAppContext,
    route: &VerificationRouteProbe,
) -> Result<(), CliRunError> {
    let mut request = RequestInput::new(
        route.method,
        built
            .runtime_plan
            .runtime
            .config
            .seo
            .canonical_host
            .as_str(),
        route.path.as_str(),
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to prepare transactional verification request `{}`: {error}",
            route.path
        ))
    })?;
    if let RouteAuthGate::Capability(capability) = route.auth {
        request = request.grant_capability(capability);
    }
    match built.runtime_plan.runtime.execute_request(
        request,
        b"01234567012345670123456701234567",
        b"76543210765432107654321076543210",
    ) {
        Ok(_) => Ok(()),
        Err(
            RequestExecutionError::SessionRequired { .. }
            | RequestExecutionError::CapabilityRequired { .. }
            | RequestExecutionError::MissingCsrfToken { .. }
            | RequestExecutionError::MissingSessionForCsrf { .. }
            | RequestExecutionError::InvalidCsrfToken { .. },
        ) => Ok(()),
        Err(other) => Err(CliRunError::execution(format!(
            "transactional verification route `{}` returned `{other}` instead of a bounded auth/csrf rejection or handler response",
            route.path
        ))),
    }
}

fn render_http_method(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Head => "HEAD",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Delete => "DELETE",
    }
}

fn reqwest_method(method: HttpMethod) -> reqwest::Method {
    match method {
        HttpMethod::Get => reqwest::Method::GET,
        HttpMethod::Head => reqwest::Method::HEAD,
        HttpMethod::Post => reqwest::Method::POST,
        HttpMethod::Put => reqwest::Method::PUT,
        HttpMethod::Patch => reqwest::Method::PATCH,
        HttpMethod::Delete => reqwest::Method::DELETE,
    }
}

fn verify_session_creation_probe(
    built: &BuiltCustomerAppContext,
    principal_id: &str,
) -> Result<(), CliRunError> {
    let server = built
        .runtime_plan
        .runtime
        .server_host(
            &EnvironmentSecretResolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build server host for session verification in `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let now = BrowserInstant::from_unix_seconds(unix_timestamp_now()?);
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .map_err(|error| CliRunError::execution(error.to_string()))?,
            now,
        )
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to issue a session for verification user `{principal_id}`: {error}"
            ))
        })?;
    let request = axum::http::Request::builder()
        .method("GET")
        .uri("/diagnostics")
        .header(
            "host",
            built
                .runtime_plan
                .runtime
                .config
                .seo
                .canonical_host
                .as_str(),
        )
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(axum::body::Body::empty())
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build session verification diagnostics request: {error}"
            ))
        })?;
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;
    let response = tokio_runtime
        .block_on(async { server.respond(request).await })
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to execute session verification request for `{principal_id}`: {error}"
            ))
        })?;
    if response.status().as_u16() != 403 {
        return Err(CliRunError::execution(format!(
            "session verification expected an authenticated diagnostics denial (403) but received {}",
            response.status().as_u16()
        )));
    }
    Ok(())
}

fn verify_fragment_rendering_probe(
    built: &BuiltCustomerAppContext,
    route: &VerificationRouteProbe,
    sample_user: Option<&String>,
) -> Result<(), CliRunError> {
    let mut request = RequestInput::new(
        HttpMethod::Get,
        built
            .runtime_plan
            .runtime
            .config
            .seo
            .canonical_host
            .as_str(),
        route.path.as_str(),
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to prepare fragment verification request `{}`: {error}",
            route.path
        ))
    })?;
    if let Some(user) = sample_user {
        request = request.with_principal(user.clone());
    }
    if let RouteAuthGate::Capability(capability) = route.auth {
        request = request.grant_capability(capability);
    }
    let execution = built
        .runtime_plan
        .runtime
        .execute_request(
            request,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to execute fragment verification route `{}`: {error}",
                route.path
            ))
        })?;
    let fragment = match &execution.response {
        HandlerResponse::Fragment(fragment) => fragment,
        other => {
            return Err(CliRunError::execution(format!(
                "fragment verification route `{}` returned `{:?}` instead of a fragment response",
                route.path, other
            )));
        }
    };
    let rendered = built
        .runtime_plan
        .runtime
        .render_fragment_response(&execution, fragment)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to render fragment verification route `{}`: {error}",
                route.path
            ))
        })?;
    if rendered.trim().is_empty() {
        return Err(CliRunError::execution(format!(
            "fragment verification route `{}` rendered empty output",
            route.path
        )));
    }
    Ok(())
}

fn verify_local_auth_failure_probe(
    built: &BuiltCustomerAppContext,
    route: &VerificationRouteProbe,
) -> Result<(), CliRunError> {
    let request = RequestInput::new(
        HttpMethod::Get,
        built
            .runtime_plan
            .runtime
            .config
            .seo
            .canonical_host
            .as_str(),
        route.path.as_str(),
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to prepare auth-failure verification request `{}`: {error}",
            route.path
        ))
    })?;
    let error = built
        .runtime_plan
        .runtime
        .execute_request(
            request,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap_err();
    match error {
        RequestExecutionError::SessionRequired { .. }
        | RequestExecutionError::CapabilityRequired { .. } => Ok(()),
        other => Err(CliRunError::execution(format!(
            "auth-failure verification route `{}` returned `{other}` instead of denying access",
            route.path
        ))),
    }
}

fn execute_cutover_observation_probe(
    built: &BuiltCustomerAppContext,
    client: &BlockingHttpClient,
    base_url: &str,
    verification: Option<&davenda_import::ImportVerification>,
    sample_routes: &[String],
    auth_probe_route: Option<&VerificationRouteProbe>,
    transactional_routes: &[VerificationRouteProbe],
    verification_checks: ObservationVerificationChecks,
    observation_started_at_unix_seconds: u64,
) -> Result<CutoverObservationProbe, CliRunError> {
    let base = Url::parse(base_url).map_err(|error| {
        CliRunError::execution(format!(
            "cutover observation base URL `{base_url}` is invalid: {error}"
        ))
    })?;
    let ready = execute_observation_json_probe(client, &base, "/ready")?;
    let health = execute_observation_json_probe(client, &base, "/health")?;
    let maintenance_enabled = health
        .body
        .get("maintenance")
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut failures = Vec::new();
    if ready.status_code != 200 || ready.status != "healthy" {
        failures.push(format!(
            "`/ready` returned {} with status `{}`",
            ready.status_code, ready.status
        ));
    }
    if health.status_code != 200 || health.status != "healthy" {
        failures.push(format!(
            "`/health` returned {} with status `{}`",
            health.status_code, health.status
        ));
    }
    if maintenance_enabled {
        failures.push("maintenance mode is unexpectedly enabled during observation".to_string());
    }

    let mut routes = Vec::new();
    let mut media_probe_count = 0usize;
    let mut observed_route_payloads = BTreeMap::new();
    for route in sample_routes {
        let url = base.join(route).map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve cutover observation route `{route}` against `{base_url}`: {error}"
            ))
        })?;
        let response = client.get(url.clone()).send().map_err(|error| {
            CliRunError::execution(format!(
                "failed to probe cutover route `{}` at `{}`: {error}",
                route, url
            ))
        })?;
        let status_code = response.status().as_u16();
        let actual_cache_headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect::<BTreeMap<_, _>>();
        let body = response.text().map_err(|error| {
            CliRunError::execution(format!(
                "failed to read cutover route `{}` at `{}`: {error}",
                route, url
            ))
        })?;
        let mut outcome = Vec::new();
        if (200..400).contains(&status_code) {
            outcome.push("healthy".to_string());
            if verification_checks.canonical_urls {
                match extract_canonical_url(&body, &url) {
                    Ok(canonical_url) => {
                        if canonical_url_matches(&canonical_url, &url) {
                            outcome.push("canonical_ok".to_string());
                        } else {
                            failures.push(format!(
                                "route `{route}` returned canonical URL `{}` instead of `{}`",
                                canonical_url, url
                            ));
                        }
                    }
                    Err(error) => failures.push(error.to_string()),
                }
            }
            if verification_checks.media_reachability {
                match extract_same_origin_media_urls(&body, &base) {
                    Ok(media_urls) => {
                        if media_urls.is_empty() {
                            outcome.push("media_none".to_string());
                        } else {
                            for media_url in &media_urls {
                                let media_response = client.get(media_url.clone()).send().map_err(|error| {
                                    CliRunError::execution(format!(
                                        "failed to probe media URL `{media_url}` for route `{route}`: {error}"
                                    ))
                                })?;
                                let media_status = media_response.status().as_u16();
                                if !(200..400).contains(&media_status) {
                                    failures.push(format!(
                                        "route `{route}` references media URL `{media_url}` that returned {} during live observation",
                                        media_status
                                    ));
                                }
                            }
                            media_probe_count += media_urls.len();
                            outcome.push(format!("media_ok({})", media_urls.len()));
                        }
                    }
                    Err(error) => failures.push(error.to_string()),
                }
            }
            match resolve_cache_route_execution(built, route) {
                Ok((execution, cache_key)) => {
                    let expected_cache_headers =
                        normalize_expected_cache_headers(&execution.cache_plan.headers);
                    let cache_mismatches = compare_expected_cache_headers(
                        &expected_cache_headers,
                        &actual_cache_headers,
                    );
                    if cache_mismatches.is_empty() {
                        outcome.push(format!("cache_ok({cache_key})"));
                    } else {
                        outcome.push("cache_mismatch".to_string());
                        failures.push(format!(
                            "cache headers for route `{route}` did not match the runtime cache policy: {}",
                            cache_mismatches.join("; ")
                        ));
                    }
                }
                Err(_error) => {
                    outcome.push("cache_skipped".to_string());
                }
            }
        } else {
            failures.push(format!(
                "route `{route}` returned unexpected status {} during live observation",
                status_code
            ));
            outcome.push("unexpected_status".to_string());
        }
        if verification_checks.cache_leaks {
            observed_route_payloads.insert(
                route.clone(),
                ObservedLiveRoutePayload {
                    status_code,
                    headers: actual_cache_headers.clone(),
                    body: body.clone(),
                },
            );
        }
        routes.push(ObservedCutoverRoute {
            route: route.clone(),
            status_code,
            outcome: outcome.join(" "),
        });
    }

    if verification_checks.auth_failures {
        let route = auth_probe_route.ok_or_else(|| {
            CliRunError::execution(
                "verification requires auth_failures but the target runtime does not expose an auth probe route"
                    .to_string(),
            )
        })?;
        let url = base.join(route.path.as_str()).map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve auth verification route `{}` against `{base_url}`: {error}",
                route.path
            ))
        })?;
        let response = client.get(url.clone()).send().map_err(|error| {
            CliRunError::execution(format!(
                "failed to probe auth verification route `{}` at `{}`: {error}",
                route.path, url
            ))
        })?;
        let status_code = response.status().as_u16();
        if status_code != 401 && status_code != 403 {
            failures.push(format!(
                "auth verification route `{}` returned {} instead of denying unauthenticated access",
                route.path, status_code
            ));
        }
        routes.push(ObservedCutoverRoute {
            route: route.path.clone(),
            status_code,
            outcome: if status_code == 401 || status_code == 403 {
                "auth_gate_ok".to_string()
            } else {
                "auth_gate_unexpected".to_string()
            },
        });
    }

    if verification_checks.transactional_journey_errors {
        let verification = verification.ok_or_else(|| {
            CliRunError::execution(
                "verification requires transactional_journey_errors but the import manifest does not declare a `[verification]` section"
                    .to_string(),
            )
        })?;
        let support = CutoverVerificationSupport {
            fragment_probe: None,
            auth_probe: None,
            transactional_probes: transactional_routes.to_vec(),
            webhook_probes: Vec::new(),
        };
        let concrete_routes = derive_transactional_probe_routes(verification, &support)?;
        for route in &concrete_routes {
            let url = base.join(route.path.as_str()).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to resolve transactional verification route `{}` against `{base_url}`: {error}",
                    route.path
                ))
            })?;
            let response = client
                .request(reqwest_method(route.method), url.clone())
                .header("content-type", "application/json")
                .body("{}")
                .send()
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to probe transactional verification route `{}` at `{}`: {error}",
                        route.path, url
                    ))
                })?;
            let status_code = response.status().as_u16();
            if status_code >= 500 || status_code == 404 || status_code == 405 {
                failures.push(format!(
                    "transactional verification route `{}` returned {} during live observation",
                    route.path, status_code
                ));
            }
            routes.push(ObservedCutoverRoute {
                route: format!("{} {}", render_http_method(route.method), route.path),
                status_code,
                outcome: if status_code >= 500 || status_code == 404 || status_code == 405 {
                    "transactional_error".to_string()
                } else {
                    format!("transactional_ok({status_code})")
                },
            });
        }
    }
    if verification_checks.cache_leaks {
        let verification = verification.ok_or_else(|| {
            CliRunError::execution(
                "verification requires cache_leaks but the import manifest does not declare a `[verification]` section"
                    .to_string(),
            )
        })?;
        observe_cache_leaks(
            built,
            client,
            &base,
            verification,
            &observed_route_payloads,
            &mut routes,
            &mut failures,
        )?;
    }
    if verification_checks.webhook_failures {
        let verification = verification.ok_or_else(|| {
            CliRunError::execution(
                "verification requires webhook_failures but the import manifest does not declare a `[verification]` section"
                    .to_string(),
            )
        })?;
        observe_webhook_failures_since(
            built,
            verification,
            observation_started_at_unix_seconds,
            &mut routes,
            &mut failures,
        )?;
    }

    if verification_checks.media_reachability && media_probe_count == 0 {
        failures.push(
            "verification requires media_reachability but no same-origin media URLs were found across the sample routes"
                .to_string(),
        );
    }

    Ok(CutoverObservationProbe {
        health_status: health.status,
        readiness_status: ready.status,
        maintenance_enabled,
        routes,
        failures,
    })
}

fn observe_cache_leaks(
    built: &BuiltCustomerAppContext,
    client: &BlockingHttpClient,
    base: &Url,
    verification: &davenda_import::ImportVerification,
    observed_routes: &BTreeMap<String, ObservedLiveRoutePayload>,
    routes: &mut Vec<ObservedCutoverRoute>,
    failures: &mut Vec<String>,
) -> Result<(), CliRunError> {
    let sample_user = verification.sample_users.first().ok_or_else(|| {
        CliRunError::execution(
            "verification check `cache_leaks` requires `[verification].sample_users`".to_string(),
        )
    })?;
    let session_cookie = issue_cutover_observation_session_cookie(built, sample_user)?;

    for route in &verification.sample_routes {
        let anonymous_before = observed_routes.get(route).ok_or_else(|| {
            CliRunError::execution(format!(
                "cache leak observation expected an anonymous probe result for route `{}`",
                route
            ))
        })?;
        let Some(anonymous_policy) = observed_public_cache_policy(&anonymous_before.headers) else {
            routes.push(ObservedCutoverRoute {
                route: format!("cache_leak {route}"),
                status_code: anonymous_before.status_code,
                outcome: "not_public_cacheable".to_string(),
            });
            continue;
        };
        let authenticated =
            execute_live_route_probe(client, base, route, Some(session_cookie.as_str()))?;
        let anonymous_after = execute_live_route_probe(client, base, route, None)?;
        let authenticated_policy = observed_public_cache_policy(&authenticated.headers);
        let mut outcome = Vec::new();
        if let Some(authenticated_policy) = authenticated_policy.as_ref() {
            let cache_mismatches =
                compare_expected_cache_headers(&anonymous_policy, authenticated_policy);
            if cache_mismatches.is_empty() {
                outcome.push("auth_public_policy_matches_anon".to_string());
            } else {
                outcome.push("auth_public_policy_changed".to_string());
            }
        } else {
            outcome.push("auth_not_public_cacheable".to_string());
        }
        if authenticated.status_code >= 500 || authenticated.status_code == 404 {
            outcome.push("auth_route_unhealthy".to_string());
            failures.push(format!(
                "cache leak observation route `{}` returned {} for authenticated traffic",
                route, authenticated.status_code
            ));
        }
        let anonymous_policy_matches_authenticated =
            authenticated_policy
                .as_ref()
                .is_some_and(|authenticated_policy| {
                    compare_expected_cache_headers(&anonymous_policy, authenticated_policy)
                        .is_empty()
                });
        if anonymous_before.body != authenticated.body && anonymous_policy_matches_authenticated {
            outcome.push("auth_body_differs_under_public_policy".to_string());
            failures.push(format!(
                "route `{}` returned different authenticated content while retaining the anonymous cache policy",
                route
            ));
        } else {
            outcome.push("auth_body_isolated".to_string());
        }
        if anonymous_after.body != anonymous_before.body {
            outcome.push("anonymous_changed_after_auth".to_string());
            if anonymous_after.body == authenticated.body {
                failures.push(format!(
                    "route `{}` served authenticated content back to anonymous traffic after the authenticated probe",
                    route
                ));
            } else {
                failures.push(format!(
                    "route `{}` changed its anonymous response after the authenticated probe, indicating unstable cache isolation",
                    route
                ));
            }
        } else {
            outcome.push("anonymous_stable".to_string());
        }
        routes.push(ObservedCutoverRoute {
            route: format!("cache_leak {route}"),
            status_code: authenticated.status_code,
            outcome: outcome.join(" "),
        });
    }

    Ok(())
}

fn issue_cutover_observation_session_cookie(
    built: &BuiltCustomerAppContext,
    principal_id: &str,
) -> Result<String, CliRunError> {
    let cookie_secret = read_runtime_secret("DAVENDA_COOKIE_SECRET")?;
    let csrf_secret = read_runtime_secret("DAVENDA_CSRF_SECRET")?;
    let allow_synthetic_session = std::env::var_os(CUTOVER_SYNTHETIC_SESSION_ENV).is_some();
    let cookie_name = built
        .runtime_plan
        .runtime
        .config
        .http
        .session_cookie
        .name
        .clone();
    let synthetic_cookie = format!("{cookie_name}=test-session-{principal_id}");
    let server = match built.runtime_plan.runtime.server_host(
        &EnvironmentSecretResolver,
        cookie_secret.as_bytes(),
        csrf_secret.as_bytes(),
    ) {
        Ok(server) => server,
        Err(error) if allow_synthetic_session => return Ok(synthetic_cookie),
        Err(error) => {
            return Err(CliRunError::execution(format!(
                "failed to build server host for cache-leak observation in `{}`: {error}",
                built.manifest.id
            )));
        }
    };
    let now = BrowserInstant::from_unix_seconds(unix_timestamp_now()?);
    let issued = match server.issue_session(
        SessionIssueRequest::new()
            .for_principal(principal_id)
            .map_err(|error| CliRunError::execution(error.to_string()))?,
        now,
    ) {
        Ok(issued) => issued,
        Err(_) if allow_synthetic_session => return Ok(synthetic_cookie),
        Err(error) => {
            return Err(CliRunError::execution(format!(
                "failed to issue a session for cache-leak observation user `{principal_id}`: {error}"
            )));
        }
    };

    Ok(format!("{}={}", cookie_name, issued.cookie_value))
}

fn execute_live_route_probe(
    client: &BlockingHttpClient,
    base: &Url,
    route: &str,
    session_cookie: Option<&str>,
) -> Result<ObservedLiveRoutePayload, CliRunError> {
    let url = base.join(route).map_err(|error| {
        CliRunError::execution(format!(
            "failed to resolve cutover observation route `{route}` against `{base}`: {error}"
        ))
    })?;
    let mut request = client.get(url.clone());
    if let Some(session_cookie) = session_cookie {
        request = request.header(reqwest::header::COOKIE, session_cookie);
    }
    let response = request.send().map_err(|error| {
        CliRunError::execution(format!(
            "failed to probe cutover route `{}` at `{}`: {error}",
            route, url
        ))
    })?;
    let status_code = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    let body = response.text().map_err(|error| {
        CliRunError::execution(format!(
            "failed to read cutover route `{}` at `{}`: {error}",
            route, url
        ))
    })?;

    Ok(ObservedLiveRoutePayload {
        status_code,
        headers,
        body,
    })
}

fn observe_webhook_failures_since(
    built: &BuiltCustomerAppContext,
    verification: &davenda_import::ImportVerification,
    observation_started_at_unix_seconds: u64,
    routes: &mut Vec<ObservedCutoverRoute>,
    failures: &mut Vec<String>,
) -> Result<(), CliRunError> {
    let snapshot = built
        .runtime_plan
        .runtime
        .wasm_host()
        .webhook_observation_snapshot(250)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to read webhook observation state for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    apply_webhook_observation_snapshot(
        verification,
        &snapshot,
        observation_started_at_unix_seconds,
        routes,
        failures,
    );
    Ok(())
}

fn apply_webhook_observation_snapshot(
    verification: &davenda_import::ImportVerification,
    snapshot: &WebhookObservationSnapshot,
    observation_started_at_unix_seconds: u64,
    routes: &mut Vec<ObservedCutoverRoute>,
    failures: &mut Vec<String>,
) {
    let recent_events = snapshot
        .recent_events
        .iter()
        .filter(|event| {
            event.recorded_at_unix_seconds >= observation_started_at_unix_seconds as i64
        })
        .cloned()
        .collect::<Vec<_>>();
    apply_webhook_observation_events(verification, recent_events.as_slice(), routes, failures);
}

fn apply_webhook_observation_events(
    verification: &davenda_import::ImportVerification,
    recent_events: &[WebhookObservationEvent],
    routes: &mut Vec<ObservedCutoverRoute>,
    failures: &mut Vec<String>,
) {
    for webhook in &verification.webhooks {
        let verification_failures = recent_events
            .iter()
            .filter(|event| {
                event.source == webhook.source
                    && event.event == webhook.event
                    && event.status == WebhookObservationStatus::VerificationFailed
            })
            .count();
        let replay_rejections = recent_events
            .iter()
            .filter(|event| {
                event.source == webhook.source
                    && event.event == webhook.event
                    && event.status == WebhookObservationStatus::ReplayRejected
            })
            .count();
        let over_budget = verification_failures > webhook.max_verification_failures as usize
            || replay_rejections > webhook.max_replay_rejections as usize;
        routes.push(ObservedCutoverRoute {
            route: format!("webhook {}/{}", webhook.source, webhook.event),
            status_code: 200,
            outcome: format!(
                "verification_failures={} replay_rejections={} {}",
                verification_failures,
                replay_rejections,
                if over_budget {
                    "threshold_exceeded"
                } else {
                    "within_budget"
                }
            ),
        });
        if verification_failures > webhook.max_verification_failures as usize {
            failures.push(format!(
                "webhook `{}/{}` observed {} verification failure(s) after observation started (max {})",
                webhook.source,
                webhook.event,
                verification_failures,
                webhook.max_verification_failures
            ));
        }
        if replay_rejections > webhook.max_replay_rejections as usize {
            failures.push(format!(
                "webhook `{}/{}` observed {} replay rejection(s) after observation started (max {})",
                webhook.source,
                webhook.event,
                replay_rejections,
                webhook.max_replay_rejections
            ));
        }
    }
}

fn extract_canonical_url(body: &str, route_url: &Url) -> Result<Url, CliRunError> {
    let lower = body.to_ascii_lowercase();
    let mut search_offset = 0usize;
    while let Some(found) = lower[search_offset..].find("<link") {
        let tag_start = search_offset + found;
        let Some(tag_end_offset) = lower[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_end_offset + 1;
        let tag = &body[tag_start..tag_end];
        let rel = extract_html_attribute_value(tag, "rel")
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_default();
        if rel.split_whitespace().any(|value| value == "canonical") {
            let href = extract_html_attribute_value(tag, "href").ok_or_else(|| {
                CliRunError::execution(format!(
                    "route `{}` declares a canonical link without an href",
                    route_url.path()
                ))
            })?;
            return resolve_relative_or_absolute_url(&href, route_url);
        }
        search_offset = tag_end;
    }

    Err(CliRunError::execution(format!(
        "route `{}` did not include a canonical URL",
        route_url.path()
    )))
}

fn extract_same_origin_media_urls(body: &str, base_url: &Url) -> Result<Vec<Url>, CliRunError> {
    let mut urls = BTreeSet::new();
    for attribute in ["src", "href"] {
        for value in extract_html_attribute_values(body, attribute) {
            if !looks_like_media_reference(&value) {
                continue;
            }
            let resolved = resolve_relative_or_absolute_url(&value, base_url)?;
            if urls_match_origin(&resolved, base_url) {
                urls.insert(resolved);
            }
        }
    }
    Ok(urls.into_iter().collect())
}

fn extract_html_attribute_values(body: &str, attribute: &str) -> Vec<String> {
    let mut values = Vec::new();
    let lower = body.to_ascii_lowercase();
    let needle = format!("{attribute}=");
    let mut offset = 0usize;
    while let Some(found) = lower[offset..].find(&needle) {
        let start = offset + found;
        if let Some(value) = extract_html_attribute_value(&body[start..], attribute) {
            values.push(value);
        }
        offset = start + needle.len();
    }
    values
}

fn extract_html_attribute_value(tag: &str, attribute: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{attribute}=");
    let found = lower.find(&needle)?;
    let value_start = found + needle.len();
    let remainder = &tag[value_start..];
    let first = remainder.chars().next()?;
    if first == '"' || first == '\'' {
        let closing = remainder[1..].find(first)?;
        return Some(remainder[1..1 + closing].to_string());
    }

    let end = remainder
        .find(|ch: char| ch.is_whitespace() || ch == '>')
        .unwrap_or(remainder.len());
    Some(remainder[..end].to_string())
}

fn resolve_relative_or_absolute_url(value: &str, base_url: &Url) -> Result<Url, CliRunError> {
    match Url::parse(value) {
        Ok(url) => Ok(url),
        Err(_) => base_url.join(value).map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve relative URL `{value}` against `{base_url}`: {error}"
            ))
        }),
    }
}

fn canonical_url_matches(actual: &Url, expected: &Url) -> bool {
    let mut actual = actual.clone();
    let mut expected = expected.clone();
    actual.set_fragment(None);
    expected.set_fragment(None);
    actual.set_query(None);
    expected.set_query(None);
    actual == expected
}

fn urls_match_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn looks_like_media_reference(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("/media/")
        || lower.contains("/assets/")
        || lower.contains("/uploads/")
        || [
            ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".avif", ".mp4", ".webm", ".mp3",
            ".pdf", ".woff", ".woff2",
        ]
        .iter()
        .any(|extension| lower.contains(extension))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservationJsonProbe {
    status_code: u16,
    status: String,
    body: Value,
}

fn execute_observation_json_probe(
    client: &BlockingHttpClient,
    base: &Url,
    path: &str,
) -> Result<ObservationJsonProbe, CliRunError> {
    let url = base.join(path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to resolve cutover observation endpoint `{path}`: {error}"
        ))
    })?;
    let response = client.get(url.clone()).send().map_err(|error| {
        CliRunError::execution(format!(
            "failed to probe cutover endpoint `{}`: {error}",
            url
        ))
    })?;
    let status_code = response.status().as_u16();
    let body = response.json::<Value>().map_err(|error| {
        CliRunError::execution(format!(
            "failed to parse JSON response from cutover endpoint `{}`: {error}",
            url
        ))
    })?;
    let status = body
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    Ok(ObservationJsonProbe {
        status_code,
        status,
        body,
    })
}

fn publication_mode_label(mode: PublicationMode) -> &'static str {
    match mode {
        PublicationMode::ValidateOnly => "validate_only",
        PublicationMode::StageValidated => "stage_validated",
        PublicationMode::PublishValidated => "publish_validated",
    }
}

fn build_customer_app_runtime_context(
    config_path: &Path,
    suppress_asset_publication: bool,
) -> Result<BuiltCustomerAppContext, CliRunError> {
    let context = load_customer_app_context(config_path)?;
    let mut config = context.config.clone();
    if suppress_asset_publication {
        config.assets.publish_manifest = false;
    }
    let runtime_plan = context
        .manifest
        .build_runtime_plan_at(
            config,
            load_auth_package_from_app_root(&context.app_root, &context.config.auth.package)?,
            context.modules,
            &context.app_root,
        )
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to build customer app runtime plan for `{}`: {error}",
                context.manifest.id
            ))
        })?;

    Ok(BuiltCustomerAppContext {
        app_root: context.app_root,
        manifest: context.manifest,
        runtime_plan,
    })
}

fn build_dev_server_runtime_plan(
    config_path: &Path,
) -> Result<davenda_runtime::RuntimePlan, CliRunError> {
    let built = build_customer_app_runtime_context(config_path, false)?;
    let mut plan = built.runtime_plan.runtime;
    ensure_dev_server_home_route(&mut plan, &built.app_root)?;
    Ok(plan)
}

fn ensure_dev_server_home_route(
    plan: &mut davenda_runtime::RuntimePlan,
    app_root: &Path,
) -> Result<(), CliRunError> {
    if plan.http.routes.iter().any(|route| route.path == "/") {
        return Ok(());
    }

    if !app_root.join("templates/pages/home.html").exists() {
        return Ok(());
    }

    let route = RouteDefinition::new("home", HttpMethod::Get, "/")
        .map_err(|error| CliRunError::execution(format!("failed to add home route: {error}")))?;
    let handler = HandlerDefinition::page("home", "pages/home")
        .map_err(|error| CliRunError::execution(format!("failed to add home handler: {error}")))?;

    plan.http.routes.push(route);
    plan.handlers.insert("home".to_string(), handler);
    Ok(())
}

fn evaluate_cutover_migration_readiness(
    config_path: &Path,
    built: &BuiltCustomerAppContext,
) -> Result<(bool, String), CliRunError> {
    let executable_plan = &built.runtime_plan.runtime.install_migrations;
    let manual_customer_entries = manual_customer_migration_entries(&built.runtime_plan);
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;
    let _runtime_guard = tokio_runtime.enter();
    let client = match built.runtime_plan.runtime.data.connect_lazy_postgres() {
        Ok(client) => client,
        Err(error) => {
            return Ok((
                false,
                format!(
                    "failed to connect to the migration database for `{}`: {error}",
                    built.manifest.id
                ),
            ));
        }
    };
    let applied_keys = tokio_runtime
        .block_on(async { client.applied_migration_keys().await })
        .map_err(|error| {
            format!(
                "failed to read applied migrations for `{}`: {error}",
                built.manifest.id
            )
        });
    let applied_keys = match applied_keys {
        Ok(applied_keys) => applied_keys,
        Err(detail) => return Ok((false, detail)),
    };
    let pending_plan = pending_migration_plan(executable_plan, &applied_keys)?;
    let pending_steps = pending_plan.ordered_steps().len();
    let (auth_ready, auth_detail) = evaluate_auth_package_validation_readiness(config_path);
    let ready = pending_steps == 0 && auth_ready && manual_customer_entries.is_empty();
    let detail = if ready {
        format!(
            "no pending executable migrations remain, auth package bindings are green, and no manual customer-app migration runbook items remain for `{}`",
            built.manifest.id
        )
    } else {
        format!(
            "{} pending executable migration steps, auth package validation is {}, and {} manual customer-app migration runbook item(s) remain for `{}`",
            pending_steps,
            if auth_ready {
                "green"
            } else {
                auth_detail.as_str()
            },
            manual_customer_entries.len(),
            built.manifest.id
        )
    };
    Ok((ready, detail))
}

fn build_cutover_check(
    id: impl Into<String>,
    description: impl Into<String>,
    required: bool,
    satisfied: bool,
) -> Result<CutoverCheck, CliRunError> {
    CutoverCheck::new(id, description, required, satisfied)
        .map_err(|error| CliRunError::execution(format!("failed to build cutover check: {error}")))
}

fn pending_migration_plan(
    plan: &MigrationPlan,
    applied_keys: &BTreeSet<(String, String)>,
) -> Result<MigrationPlan, CliRunError> {
    let mut pending = MigrationPlan::new();
    for step in plan.ordered_steps() {
        let key = (step.owner.to_string(), step.id.to_string());
        if applied_keys.contains(&key) {
            continue;
        }
        pending.insert(step.clone()).map_err(|error| {
            CliRunError::execution(format!(
                "failed to stage pending migration `{}` for apply: {error}",
                step.id
            ))
        })?;
    }
    Ok(pending)
}

fn build_migrate_apply_report(
    manifest: &CustomerAppManifest,
    executable_plan: &MigrationPlan,
    applied_keys: Option<&BTreeSet<(String, String)>>,
    executed_statements: Option<usize>,
    manual_customer_migration_entries: &[MigrationPlanEntry],
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let pending_steps = executable_plan
        .ordered_steps()
        .iter()
        .filter(|step| {
            applied_keys
                .is_none_or(|keys| !keys.contains(&(step.owner.to_string(), step.id.to_string())))
        })
        .count();
    let total_steps = executable_plan.ordered_steps().len();
    let already_applied = total_steps.saturating_sub(pending_steps);
    let planned_sql_statements = if pending_steps == 0 {
        0
    } else {
        1 + executable_plan
            .ordered_steps()
            .iter()
            .filter(|step| {
                applied_keys.is_none_or(|keys| {
                    !keys.contains(&(step.owner.to_string(), step.id.to_string()))
                })
            })
            .map(|step| step.statements.len() + 1)
            .sum::<usize>()
    };

    let summary = if dry_run {
        if total_steps == 0 {
            format!(
                "No executable SQL migrations are defined for customer app `{}`",
                manifest.id
            )
        } else {
            format!(
                "Planned migration apply for `{}` with {} executable steps and {} SQL statements",
                manifest.id, pending_steps, planned_sql_statements
            )
        }
    } else if pending_steps == 0 {
        format!(
            "No pending executable migrations remained for customer app `{}`",
            manifest.id
        )
    } else {
        format!(
            "Applied {} executable migration steps for `{}` with {} SQL statements",
            pending_steps,
            manifest.id,
            executed_statements.unwrap_or_default()
        )
    };

    let mut report = CommandReport::new(["migrate", "apply"], summary)
        .map_err(report_build_error)?
        .with_columns([
            "owner",
            "step",
            "order",
            "online_safe",
            "sql_statements",
            "status",
            "description",
        ])
        .map_err(report_build_error)?;
    if executable_plan
        .ordered_steps()
        .iter()
        .any(|step| !step.online_safe)
    {
        report = report.with_status(ReportStatus::Warning);
    }

    for step in executable_plan.ordered_steps() {
        let already_applied_step = applied_keys
            .is_some_and(|keys| keys.contains(&(step.owner.to_string(), step.id.to_string())));
        let status = if already_applied_step {
            "already_applied"
        } else if dry_run {
            "planned"
        } else {
            "applied"
        };
        report.push_row(
            ReportRow::new()
                .with_cell("owner", step.owner.to_string())
                .map_err(report_build_error)?
                .with_cell("step", step.id.to_string())
                .map_err(report_build_error)?
                .with_cell("order", step.order.to_string())
                .map_err(report_build_error)?
                .with_cell("online_safe", step.online_safe.to_string())
                .map_err(report_build_error)?
                .with_cell("sql_statements", step.statements.len().to_string())
                .map_err(report_build_error)?
                .with_cell("status", status)
                .map_err(report_build_error)?
                .with_cell("description", step.description.clone())
                .map_err(report_build_error)?,
        );
    }

    let validation_status = if dry_run { "planned" } else { "validated" };
    report.push_row(
        ReportRow::new()
            .with_cell("owner", format!("auth:{}", manifest.auth.package_name))
            .map_err(report_build_error)?
            .with_cell("step", "validate")
            .map_err(report_build_error)?
            .with_cell("order", "0")
            .map_err(report_build_error)?
            .with_cell("online_safe", "true")
            .map_err(report_build_error)?
            .with_cell("sql_statements", "0")
            .map_err(report_build_error)?
            .with_cell("status", validation_status)
            .map_err(report_build_error)?
            .with_cell(
                "description",
                format!(
                    "validate auth package `{}` schema, model, and capability bindings against the installed modules",
                    manifest.auth.package_name
                ),
            )
            .map_err(report_build_error)?,
    );
    report.push_row(
        ReportRow::new()
            .with_cell("owner", format!("customer_app:{}", manifest.id))
            .map_err(report_build_error)?
            .with_cell("step", "validate")
            .map_err(report_build_error)?
            .with_cell("order", "0")
            .map_err(report_build_error)?
            .with_cell("online_safe", "true")
            .map_err(report_build_error)?
            .with_cell("sql_statements", "0")
            .map_err(report_build_error)?
            .with_cell("status", validation_status)
            .map_err(report_build_error)?
            .with_cell(
                "description",
                format!(
                    "validate customer app `{}` root, manifest/config alignment, and runtime composition before release",
                    manifest.id
                ),
            )
            .map_err(report_build_error)?,
    );

    for entry in manual_customer_migration_entries {
        report.push_row(
            ReportRow::new()
                .with_cell("owner", release_plan_owner_label(&entry.owner))
                .map_err(report_build_error)?
                .with_cell("step", "manual-runbook")
                .map_err(report_build_error)?
                .with_cell("order", entry.order.to_string())
                .map_err(report_build_error)?
                .with_cell("online_safe", entry.online_safe.to_string())
                .map_err(report_build_error)?
                .with_cell("sql_statements", "0")
                .map_err(report_build_error)?
                .with_cell("status", "manual")
                .map_err(report_build_error)?
                .with_cell("description", entry.description.clone())
                .map_err(report_build_error)?,
        );
    }

    if already_applied > 0 {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Info,
            "migrate.steps.already_applied",
            format!(
                "{} executable migration steps were already present in the migration ledger and were not replayed",
                already_applied
            ),
        )?;
    }
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        if dry_run {
            "migrate.auth_package.validation.planned"
        } else {
            "migrate.auth_package.validation"
        },
        format!(
            "{} auth package `{}` schema, model, and capability bindings {}",
            if dry_run { "planned" } else { "validated" },
            manifest.auth.package_name,
            if dry_run {
                "before applying the target release"
            } else {
                "as part of migrate apply"
            }
        ),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        if dry_run {
            "migrate.customer_app.validation.planned"
        } else {
            "migrate.customer_app.validation"
        },
        format!(
            "{} customer app `{}` root, manifest/config alignment, and runtime composition {}",
            if dry_run { "planned" } else { "validated" },
            manifest.id,
            if dry_run {
                "before the release is applied"
            } else {
                "during migrate apply"
            }
        ),
    )?;
    if !manual_customer_migration_entries.is_empty() {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "migrate.customer_app.manual_runbook",
            format!(
                "{} customer-app migration runbook step(s) remain manual because they do not compile into executable SQL or built-in validation steps",
                manual_customer_migration_entries.len()
            ),
        )?;
    }

    Ok(report)
}

fn build_assets_publish_report(
    manifest: &CustomerAppManifest,
    active_manifest: &davenda_assets::ActiveAssetManifest,
    receipt: Option<&davenda_assets::ThemeAssetPublicationReceipt>,
    dry_run: bool,
) -> Result<CommandReport, CliRunError> {
    let summary = if dry_run {
        format!(
            "Planned theme asset publication for `{}` with {} artifacts",
            manifest.id,
            active_manifest.entries().len()
        )
    } else {
        format!(
            "Published theme assets for `{}` with {} artifacts",
            manifest.id,
            active_manifest.entries().len()
        )
    };
    let mut report = CommandReport::new(["assets", "publish"], summary)
        .map_err(report_build_error)?
        .with_columns([
            "logical_path",
            "hashed_path",
            "delivery",
            "bytes",
            "status",
            "storage_path",
        ])
        .map_err(report_build_error)?;

    let mut total_bytes = 0_u64;
    for (index, (logical_path, published)) in active_manifest.entries().enumerate() {
        let write = receipt.and_then(|value| value.writes().get(index));
        total_bytes += published.artifact().byte_length();
        report.push_row(
            ReportRow::new()
                .with_cell("logical_path", logical_path)
                .map_err(report_build_error)?
                .with_cell("hashed_path", published.artifact().hashed_path())
                .map_err(report_build_error)?
                .with_cell(
                    "delivery",
                    format_asset_delivery_target(published.delivery().target()),
                )
                .map_err(report_build_error)?
                .with_cell("bytes", published.artifact().byte_length().to_string())
                .map_err(report_build_error)?
                .with_cell("status", if dry_run { "planned" } else { "published" })
                .map_err(report_build_error)?
                .with_cell(
                    "storage_path",
                    write
                        .map(|receipt| receipt.path.display().to_string())
                        .unwrap_or_else(|| "not_written".to_string()),
                )
                .map_err(report_build_error)?,
        );
    }

    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "assets.bytes.total",
        format!(
            "theme asset publication covers {} artifacts and {} bytes",
            active_manifest.entries().len(),
            total_bytes
        ),
    )?;

    Ok(report)
}

fn manual_customer_migration_entries(
    runtime_plan: &CustomerAppRuntimePlan,
) -> Vec<MigrationPlanEntry> {
    runtime_plan
        .migration_summary
        .entries()
        .iter()
        .filter(|entry| {
            entry.step_id.is_none() && matches!(entry.owner, MigrationPlanOwner::CustomerApp(_))
        })
        .cloned()
        .collect()
}

fn evaluate_auth_package_validation_readiness(config_path: &Path) -> (bool, String) {
    match run_auth_package_validate(&AuthPackageValidateInvocation {
        config_path: config_path.to_path_buf(),
    }) {
        Ok(report) if report.status != ReportStatus::Unsafe => (true, "green".to_string()),
        Ok(report) => (false, report.summary),
        Err(error) => (false, error.to_string()),
    }
}

fn format_asset_delivery_target(target: &AssetDeliveryTarget) -> String {
    match target {
        AssetDeliveryTarget::Cdn { public_url, .. } => public_url.clone(),
        AssetDeliveryTarget::SignedObject { object_key } => format!("signed:{object_key}"),
        AssetDeliveryTarget::AppProxy { path } => format!("app:{path}"),
        AssetDeliveryTarget::LocalPath { path } => format!("local:{path}"),
    }
}

fn storage_class_label(class: StorageClass) -> &'static str {
    match class {
        StorageClass::PublicAsset => "public_asset",
        StorageClass::PublicUpload => "public_upload",
        StorageClass::PrivateShared => "private_shared",
        StorageClass::LocalOnlySensitive => "local_only_sensitive",
    }
}

fn storage_policy_label(plan: &davenda_storage::StoragePlan) -> String {
    format!(
        "{:?}/{:?}/{:?}",
        plan.policy.delivery_mode, plan.policy.sync_mode, plan.policy.sensitivity
    )
}

fn push_report_diagnostic(
    report: &mut CommandReport,
    severity: DiagnosticSeverity,
    code: &str,
    message: impl Into<String>,
) -> Result<(), CliRunError> {
    report.push_diagnostic(
        DiagnosticRecord::new(severity, code, message.into()).map_err(report_build_error)?,
    );
    Ok(())
}

fn report_build_error(error: impl std::fmt::Display) -> CliRunError {
    CliRunError::execution(format!("failed to build command report: {error}"))
}

fn import_model_error(error: ImportModelError) -> CliRunError {
    CliRunError::execution(format!("failed to build import model data: {error}"))
}

fn run_dev_server(invocation: &DevServerInvocation) -> Result<(), CliRunError> {
    let cookie_secret = read_runtime_secret("DAVENDA_COOKIE_SECRET")?;
    let csrf_secret = read_runtime_secret("DAVENDA_CSRF_SECRET")?;
    let plan = build_dev_server_runtime_plan(&invocation.config_path)?;
    let bind = plan.config.server.bind.clone();
    let tokio_runtime = build_dev_server_async_runtime()?;
    let runtime_guard = tokio_runtime.enter();
    let server = plan
        .server_host(
            &EnvironmentSecretResolver,
            cookie_secret.as_bytes(),
            csrf_secret.as_bytes(),
        )
        .map_err(|error| {
            CliRunError::execution(format!("failed to build dev server host: {error}"))
        })?;
    drop(runtime_guard);
    let app_name = plan.config.app.name.clone();

    tokio_runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(&bind)
            .await
            .map_err(|error| {
                CliRunError::execution(format!("failed to bind dev server on `{bind}`: {error}"))
            })?;

        println!("Serving `{}` on http://{bind}", app_name);
        server.serve(listener).await.map_err(|error| {
            CliRunError::execution(format!("dev server stopped unexpectedly: {error}"))
        })
    })
}

fn read_runtime_secret(var: &str) -> Result<String, CliRunError> {
    std::env::var(var).map_err(|_| {
        CliRunError::execution(format!(
            "missing `{var}`; set it before starting `dev server`"
        ))
    })
}

fn unix_timestamp_now() -> Result<u64, CliRunError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| CliRunError::execution(format!("failed to calculate timestamp: {error}")))
}

fn import_journal_path(manifest_path: &Path, run_id: &impl std::fmt::Display) -> PathBuf {
    let parent = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    parent
        .join(".davenda")
        .join("import-runs")
        .join(format!("{run_id}.json"))
}

fn cutover_journal_path(manifest_path: &Path, run_id: &impl std::fmt::Display) -> PathBuf {
    let parent = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    parent
        .join(".davenda")
        .join("cutover-runs")
        .join(format!("{run_id}.json"))
}

fn build_import_runtime_context(
    manifest_root: &Path,
    manifest: &ImportManifest,
) -> Result<Option<BuiltImportRuntimeContext>, CliRunError> {
    let Some(target) = manifest.target.as_ref() else {
        return Ok(None);
    };

    let config_path = manifest_root.join(&target.platform_config);
    let built = build_customer_app_runtime_context(&config_path, true)?;
    if built.manifest.id.as_str() != manifest.customer_app_id {
        return Err(CliRunError::execution(format!(
            "import manifest targets customer app `{}`, but runtime config resolves `{}`",
            manifest.customer_app_id, built.manifest.id
        )));
    }

    let resolved_manifest = CustomerAppManifest::from_file(
        manifest_root.join(&target.app_manifest),
    )
    .map_err(|error| {
        CliRunError::execution(format!(
            "failed to load import target app manifest `{}`: {error}",
            manifest_root.join(&target.app_manifest).display()
        ))
    })?;
    if resolved_manifest.id != built.manifest.id {
        return Err(CliRunError::execution(format!(
            "import target app manifest `{}` resolves `{}`, but runtime config resolves `{}`",
            target.app_manifest, resolved_manifest.id, built.manifest.id
        )));
    }

    let installed_modules = built
        .manifest
        .modules
        .iter()
        .map(|module| module.id.to_string())
        .collect::<BTreeSet<_>>();
    let missing_modules = manifest
        .modules
        .iter()
        .filter(|module| !installed_modules.contains(module.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_modules.is_empty() {
        return Err(CliRunError::execution(format!(
            "import manifest requires modules not installed in `{}`: {}",
            built.manifest.id,
            missing_modules.join(", ")
        )));
    }
    let missing_expected_modules = target
        .expected_modules
        .iter()
        .filter(|module| !installed_modules.contains(module.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_expected_modules.is_empty() {
        return Err(CliRunError::execution(format!(
            "import target expects modules not installed in `{}`: {}",
            built.manifest.id,
            missing_expected_modules.join(", ")
        )));
    }

    if let Some(locale) = manifest.locale.as_deref() {
        let supported = built
            .manifest
            .supported_locales
            .iter()
            .any(|candidate| candidate.as_str() == locale);
        if !supported {
            return Err(CliRunError::execution(format!(
                "import manifest locale `{locale}` is not supported by customer app `{}`",
                built.manifest.id
            )));
        }
    }

    let object_store = built
        .runtime_plan
        .runtime
        .object_store_client_config(&EnvironmentSecretResolver)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve storage backends for import target `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let storage_host = built
        .runtime_plan
        .runtime
        .storage_host_with_object_store(object_store);

    Ok(Some(BuiltImportRuntimeContext {
        built,
        storage_host,
    }))
}

fn build_import_auth_context(
    runtime: &BuiltImportRuntimeContext,
    manifest_root: &Path,
    manifest: &ImportManifest,
    require_auth_mapping: bool,
    tokio_runtime: &tokio::runtime::Runtime,
) -> Result<LiveImportAuthContext, CliRunError> {
    let client = runtime
        .built
        .runtime_plan
        .runtime
        .data
        .connect_lazy_postgres()
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to connect live auth import backend for `{}`: {error}",
                runtime.built.manifest.id
            ))
        })?;
    let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
    let auth = DavendaAuth::new(engine, runtime.built.runtime_plan.runtime.tenant_id());
    let auth_package_name = runtime
        .built
        .runtime_plan
        .runtime
        .config
        .auth
        .package
        .clone();
    let auth_package =
        load_auth_package_from_app_root(&runtime.built.app_root, &auth_package_name)?;
    tokio_runtime
        .block_on(async { auth.apply_model_package(&auth_package).await })
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to apply auth model package `{}` for live import: {error}",
                auth_package_name
            ))
        })?;
    let auth_mapping = if require_auth_mapping {
        let mapping = manifest.load_auth_mapping(manifest_root).map_err(|error| {
            CliRunError::execution(format!(
                "failed to load import auth mapping document for `{}`: {error}",
                manifest_root.display()
            ))
        })?;
        validate_import_auth_mapping(&mapping, &auth_package).map_err(|error| {
            CliRunError::execution(format!(
                "failed to validate import auth mapping for `{}`: {error}",
                manifest_root.display()
            ))
        })?;
        Some(mapping)
    } else {
        None
    };

    Ok(LiveImportAuthContext {
        auth,
        site_id: manifest.site.clone(),
        storefront_id: runtime.built.manifest.id.to_string(),
        auth_package: AuthModelPackageSelection::new(auth_package),
        auth_mapping,
    })
}

fn validate_import_auth_mapping(
    auth_mapping: &ImportAuthMapping,
    auth_package: &dyn AuthModelPackage,
) -> Result<(), ImportModelError> {
    for role_mapping in auth_mapping.role_mappings() {
        for capability_name in &role_mapping.capabilities {
            let capability = Capability::from_str(capability_name).ok_or_else(|| {
                ImportModelError::ManifestParse {
                    message: format!(
                        "auth mapping role `{}` references unsupported capability `{}`",
                        role_mapping.legacy_role, capability_name
                    ),
                }
            })?;
            if auth_package.binding_for(capability).is_none() {
                return Err(ImportModelError::ManifestParse {
                    message: format!(
                        "auth mapping role `{}` references capability `{}` which is not bound by auth package `{}`",
                        role_mapping.legacy_role,
                        capability.as_str(),
                        auth_package.manifest().name
                    ),
                });
            }
        }
    }

    Ok(())
}

fn materialize_import_assets(
    report: &mut CommandReport,
    manifest_root: &Path,
    runtime: &BuiltImportRuntimeContext,
    execution: &davenda_import::ImportExecution,
) -> Result<(), CliRunError> {
    for record in &execution.importer_records {
        if record.resource_kind != "asset" {
            continue;
        }
        let Some(staged_path) = record.staged_path.as_ref() else {
            continue;
        };
        let staged_path = PathBuf::from(staged_path);
        let input = fs::read_to_string(&staged_path).map_err(|error| {
            CliRunError::execution(format!(
                "failed to read staged asset import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;
        let records: Vec<Value> = serde_json::from_str(&input).map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse staged asset import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;
        let materialized = records
            .iter()
            .filter(|record| {
                record
                    .get("normalized")
                    .and_then(Value::as_object)
                    .and_then(|normalized| normalized.get("materialized"))
                    .is_some()
            })
            .count();
        if materialized == 0 {
            continue;
        }
        report.push_diagnostic(
            DiagnosticRecord::new(
                DiagnosticSeverity::Info,
                format!("import.{}.materialized", record.importer_id),
                format!(
                    "materialized {materialized} imported assets into runtime storage for `{}` from `{}`",
                    runtime.built.manifest.id,
                    manifest_root.display()
                ),
            )
            .map_err(report_build_error)?,
        );
    }
    Ok(())
}

fn materialize_import_pages(
    report: &mut CommandReport,
    manifest_root: &Path,
    runtime: &BuiltImportRuntimeContext,
    execution: &davenda_import::ImportExecution,
) -> Result<(), CliRunError> {
    let mut page_counts = HashMap::<String, usize>::new();

    for record in &execution.importer_records {
        if record.resource_kind != "page" {
            continue;
        }
        let Some(staged_path) = record.staged_path.as_ref() else {
            continue;
        };
        let staged_path = PathBuf::from(staged_path);
        let input = fs::read_to_string(&staged_path).map_err(|error| {
            CliRunError::execution(format!(
                "failed to read staged page import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;
        let records: Vec<Value> = serde_json::from_str(&input).map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse staged page import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;

        for table in records.iter().filter_map(|record| {
            record
                .get("normalized")
                .and_then(Value::as_object)
                .and_then(|normalized| normalized.get("persisted"))
                .and_then(Value::as_object)
                .and_then(|persisted| persisted.get("table"))
                .and_then(Value::as_str)
        }) {
            *page_counts.entry(table.to_string()).or_insert(0) += 1;
        }
    }

    for (table, count) in page_counts {
        report.push_diagnostic(
            DiagnosticRecord::new(
                DiagnosticSeverity::Info,
                "import.page.persisted",
                format!(
                    "persisted {count} imported pages into `{table}` for `{}` from `{}`",
                    runtime.built.manifest.id,
                    manifest_root.display()
                ),
            )
            .map_err(report_build_error)?,
        );
    }

    Ok(())
}

fn materialize_import_events(
    report: &mut CommandReport,
    manifest_root: &Path,
    runtime: &BuiltImportRuntimeContext,
    execution: &davenda_import::ImportExecution,
) -> Result<(), CliRunError> {
    let mut event_counts = HashMap::<String, usize>::new();

    for record in &execution.importer_records {
        if record.resource_kind != "event" {
            continue;
        }
        let Some(staged_path) = record.staged_path.as_ref() else {
            continue;
        };
        let staged_path = PathBuf::from(staged_path);
        let input = fs::read_to_string(&staged_path).map_err(|error| {
            CliRunError::execution(format!(
                "failed to read staged event import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;
        let records: Vec<Value> = serde_json::from_str(&input).map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse staged event import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;

        for table in records.iter().filter_map(|record| {
            record
                .get("normalized")
                .and_then(Value::as_object)
                .and_then(|normalized| normalized.get("persisted"))
                .and_then(Value::as_object)
                .and_then(|persisted| persisted.get("table"))
                .and_then(Value::as_str)
        }) {
            *event_counts.entry(table.to_string()).or_insert(0) += 1;
        }
    }

    for (table, count) in event_counts {
        report.push_diagnostic(
            DiagnosticRecord::new(
                DiagnosticSeverity::Info,
                "import.event.persisted",
                format!(
                    "persisted {count} imported events into `{table}` for `{}` from `{}`",
                    runtime.built.manifest.id,
                    manifest_root.display()
                ),
            )
            .map_err(report_build_error)?,
        );
    }

    Ok(())
}

fn materialize_import_users(
    report: &mut CommandReport,
    manifest_root: &Path,
    runtime: &BuiltImportRuntimeContext,
    execution: &davenda_import::ImportExecution,
) -> Result<(), CliRunError> {
    let mut user_counts = HashMap::<String, usize>::new();

    for record in &execution.importer_records {
        if record.resource_kind != "user" {
            continue;
        }
        let Some(staged_path) = record.staged_path.as_ref() else {
            continue;
        };
        let staged_path = PathBuf::from(staged_path);
        let input = fs::read_to_string(&staged_path).map_err(|error| {
            CliRunError::execution(format!(
                "failed to read staged user import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;
        let records: Vec<Value> = serde_json::from_str(&input).map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse staged user import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;

        for table in records.iter().flat_map(persisted_tables) {
            *user_counts.entry(table).or_insert(0) += 1;
        }
    }

    for (table, count) in user_counts {
        report.push_diagnostic(
            DiagnosticRecord::new(
                DiagnosticSeverity::Info,
                "import.user.persisted",
                format!(
                    "persisted {count} imported users into `{table}` for `{}` from `{}`",
                    runtime.built.manifest.id,
                    manifest_root.display()
                ),
            )
            .map_err(report_build_error)?,
        );
    }

    Ok(())
}

fn materialize_import_memberships(
    report: &mut CommandReport,
    manifest_root: &Path,
    runtime: &BuiltImportRuntimeContext,
    execution: &davenda_import::ImportExecution,
) -> Result<(), CliRunError> {
    let mut membership_counts = HashMap::<String, usize>::new();

    for record in &execution.importer_records {
        if !matches!(
            record.resource_kind.as_str(),
            "membership_tier" | "subscription"
        ) {
            continue;
        }
        let Some(staged_path) = record.staged_path.as_ref() else {
            continue;
        };
        let staged_path = PathBuf::from(staged_path);
        let input = fs::read_to_string(&staged_path).map_err(|error| {
            CliRunError::execution(format!(
                "failed to read staged membership import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;
        let records: Vec<Value> = serde_json::from_str(&input).map_err(|error| {
            CliRunError::execution(format!(
                "failed to parse staged membership import artifact `{}`: {error}",
                staged_path.display()
            ))
        })?;

        for table in records.iter().flat_map(persisted_tables) {
            *membership_counts.entry(table).or_insert(0) += 1;
        }
    }

    for (table, count) in membership_counts {
        report.push_diagnostic(
            DiagnosticRecord::new(
                DiagnosticSeverity::Info,
                "import.membership.persisted",
                format!(
                    "persisted {count} imported membership records into `{table}` for `{}` from `{}`",
                    runtime.built.manifest.id,
                    manifest_root.display()
                ),
            )
            .map_err(report_build_error)?,
        );
    }

    Ok(())
}

fn persisted_tables(record: &Value) -> Vec<String> {
    let Some(normalized) = record.get("normalized").and_then(Value::as_object) else {
        return Vec::new();
    };
    let Some(persisted) = normalized.get("persisted") else {
        return Vec::new();
    };

    match persisted {
        Value::Object(object) => object
            .get("table")
            .and_then(Value::as_str)
            .map(|table| vec![table.to_string()])
            .unwrap_or_default(),
        Value::Array(entries) => entries
            .iter()
            .filter_map(|entry| {
                entry
                    .get("table")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn materialize_asset_record(
    storage_host: &StorageHost,
    asset_storage_default: davenda_import::AssetStorageDefault,
    manifest_root: &Path,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let checksum = staged_record
        .get("checksum")
        .and_then(Value::as_str)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged asset record is missing `checksum`".to_string(),
        })?
        .to_string();
    let target_id = staged_record
        .get("target_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged asset record is missing `target_id`".to_string(),
        })?
        .to_string();
    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged asset record is missing `normalized` object data".to_string(),
        })?;
    let source_file = normalized
        .get("source_file")
        .and_then(Value::as_str)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged asset record is missing `normalized.source_file`".to_string(),
        })?;
    let source_path = manifest_root.join(source_file);
    let bytes = fs::read(&source_path).map_err(|error| ImportModelError::SourceRead {
        importer_id: "asset".to_string(),
        path: source_path.display().to_string(),
        message: error.to_string(),
    })?;
    let logical_path = normalized
        .get("logical_path")
        .and_then(Value::as_str)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged asset record is missing `logical_path`".to_string(),
        })?;
    let content_type = normalized
        .get("content_type")
        .and_then(Value::as_str)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged asset record is missing `content_type`".to_string(),
        })?;

    let override_policy = storage_override_for_import_default(asset_storage_default);
    let revision = storage_host
        .plan_managed_revision(
            RevisionId::new(format!("rev-{target_id}")).map_err(|error| {
                ImportModelError::ManifestParse {
                    message: error.to_string(),
                }
            })?,
            logical_path,
            override_policy,
            content_type,
            bytes.len() as u64,
            ContentFingerprint::new(FingerprintAlgorithm::Sha256, checksum.clone()).map_err(
                |error| ImportModelError::ManifestParse {
                    message: error.to_string(),
                },
            )?,
        )
        .map_err(|error| ImportModelError::ManifestParse {
            message: error.to_string(),
        })?;
    let write = storage_host
        .execute_write_with_content_type(revision.storage_plan(), &bytes, Some(content_type))
        .map_err(|error| ImportModelError::ManifestParse {
            message: error.to_string(),
        })?;
    let delivery = storage_host
        .delivery_location(revision.storage_plan())
        .map_err(|error| ImportModelError::ManifestParse {
            message: error.to_string(),
        })?;

    normalized.insert(
        "materialized".to_string(),
        serde_json::json!({
            "bytes_written": write.bytes_written,
            "path": write.path.display().to_string(),
            "delivery": render_storage_delivery(&delivery),
        }),
    );
    Ok(())
}

fn ensure_import_data_client(
    data_runtime: &davenda_data::DataRuntime,
    client: &mut Option<PostgresDataClient>,
) -> Result<PostgresDataClient, ImportModelError> {
    if let Some(client) = client.as_ref() {
        return Ok(client.clone());
    }

    let connected =
        data_runtime
            .connect_lazy_postgres()
            .map_err(|error| ImportModelError::ManifestParse {
                message: format!("failed to connect live import data client: {error}"),
            })?;
    *client = Some(connected.clone());
    Ok(connected)
}

fn materialize_page_record(
    tokio_runtime: &tokio::runtime::Runtime,
    data_client: &PostgresDataClient,
    default_locale: &str,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (mutation, persisted) = page_import_mutation(staged_record, default_locale)?;
    let statement = mutation.compile(1).map_err(import_data_model_error)?;
    tokio_runtime
        .block_on(async { data_client.execute_statement(&statement).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to persist imported page: {error}"),
        })?;

    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged page record is missing `normalized` object data".to_string(),
        })?;
    normalized.insert("persisted".to_string(), persisted);
    Ok(())
}

fn materialize_event_record(
    tokio_runtime: &tokio::runtime::Runtime,
    data_client: &PostgresDataClient,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (mutation, persisted) = event_import_mutation(staged_record)?;
    let statement = mutation.compile(1).map_err(import_data_model_error)?;
    tokio_runtime
        .block_on(async { data_client.execute_statement(&statement).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to persist imported event: {error}"),
        })?;

    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged event record is missing `normalized` object data".to_string(),
        })?;
    normalized.insert("persisted".to_string(), persisted);
    Ok(())
}

fn materialize_user_record(
    tokio_runtime: &tokio::runtime::Runtime,
    data_runtime: &DataRuntime,
    data_client: &PostgresDataClient,
    auth_context: &LiveImportAuthContext,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (mutation, account_persisted) = user_account_import_mutation(staged_record)?;
    execute_membership_import_transaction(
        tokio_runtime,
        data_runtime,
        data_client,
        "import.membership.user",
        &[("membership_member_accounts", "upsert")],
        vec![mutation],
        "failed to persist imported user account state",
    )?;

    let auth_mapping =
        auth_context
            .auth_mapping
            .as_ref()
            .ok_or_else(|| ImportModelError::ManifestParse {
                message: "live user import requires a loaded auth mapping document".to_string(),
            })?;
    let (updates, auth_persisted) = user_import_updates(
        staged_record,
        auth_context.site_id.as_deref(),
        &auth_context.storefront_id,
        auth_context.auth_package.package(),
        auth_mapping,
    )?;
    tokio_runtime
        .block_on(async { auth_context.auth.write(updates).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to persist imported user auth state: {error}"),
        })?;

    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged user record is missing `normalized` object data".to_string(),
        })?;
    normalized.insert(
        "persisted".to_string(),
        serde_json::json!([account_persisted, auth_persisted]),
    );
    Ok(())
}

fn materialize_membership_tier_record(
    tokio_runtime: &tokio::runtime::Runtime,
    data_client: &PostgresDataClient,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (mutation, persisted) = membership_tier_import_mutation(staged_record)?;
    let statement = mutation.compile(1).map_err(import_data_model_error)?;
    tokio_runtime
        .block_on(async { data_client.execute_statement(&statement).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to persist imported membership tier: {error}"),
        })?;

    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged membership tier record is missing `normalized` object data"
                .to_string(),
        })?;
    normalized.insert("persisted".to_string(), persisted);
    Ok(())
}

fn materialize_subscription_record(
    tokio_runtime: &tokio::runtime::Runtime,
    data_runtime: &DataRuntime,
    data_client: &PostgresDataClient,
    auth_context: &LiveImportAuthContext,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (account_mutation, mut account_persisted) =
        subscription_member_account_bootstrap_mutation(staged_record)?;

    let (mutations, auth_updates, persisted) =
        subscription_import_persistence(staged_record, &auth_context.storefront_id)?;
    execute_membership_import_transaction(
        tokio_runtime,
        data_runtime,
        data_client,
        "import.membership.subscription",
        &[
            ("membership_member_accounts", "upsert"),
            ("membership_subscriptions", "upsert"),
            ("membership_entitlements", "upsert"),
        ],
        std::iter::once(account_mutation).chain(mutations).collect(),
        "failed to persist imported subscription state",
    )?;
    tokio_runtime
        .block_on(async { auth_context.auth.write(auth_updates).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to persist imported subscription auth state: {error}"),
        })?;

    if let Some(account_persisted) = account_persisted.as_object_mut() {
        account_persisted.insert(
            "disposition".to_string(),
            serde_json::json!("transactional"),
        );
    }

    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged subscription record is missing `normalized` object data".to_string(),
        })?;
    let persisted = match persisted {
        Value::Array(mut entries) => {
            entries.insert(0, account_persisted);
            Value::Array(entries)
        }
        other => Value::Array(vec![account_persisted, other]),
    };
    normalized.insert("persisted".to_string(), persisted);
    Ok(())
}

fn execute_membership_import_transaction(
    tokio_runtime: &tokio::runtime::Runtime,
    data_runtime: &DataRuntime,
    data_client: &PostgresDataClient,
    transaction_name: &str,
    writes: &[(&str, &str)],
    mutations: Vec<MutationSpec>,
    error_context: &str,
) -> Result<(), ImportModelError> {
    let compiled =
        compile_membership_import_transaction(data_runtime, transaction_name, writes, mutations)?;
    tokio_runtime
        .block_on(async { data_client.execute_transaction(&compiled).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("{error_context}: {error}"),
        })?;
    Ok(())
}

fn compile_membership_import_transaction(
    data_runtime: &DataRuntime,
    transaction_name: &str,
    writes: &[(&str, &str)],
    mutations: Vec<MutationSpec>,
) -> Result<CompiledTransaction, ImportModelError> {
    let mut plan = TransactionPlan::new(transaction_name, TransactionIsolation::Serializable)
        .map_err(import_data_model_error)?;
    for (resource, action) in writes {
        plan =
            plan.with_write(DomainWrite::new(*resource, *action).map_err(import_data_model_error)?);
    }
    data_runtime
        .compile_transaction(&plan, &mutations)
        .map_err(import_data_model_error)
}

fn subscription_member_account_bootstrap_mutation(
    staged_record: &Value,
) -> Result<(MutationSpec, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged subscription record is missing `normalized` object data".to_string(),
        })?;
    let principal_id = required_normalized_string(normalized, "principal_id")?;
    MemberAccountId::new(principal_id.clone()).map_err(import_membership_model_error)?;
    let email = optional_normalized_string(normalized, "email")?.unwrap_or_default();
    let username =
        optional_normalized_string(normalized, "username")?.unwrap_or_else(|| principal_id.clone());
    let display_name =
        optional_normalized_string(normalized, "display_name")?.unwrap_or_else(|| username.clone());
    let synthetic_source_key = format!("{source_key}#member-account:{principal_id}");
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate member account bootstrap timestamp: {error}"),
        })?
        .as_secs();

    let mutation = MutationSpec::new("membership_member_accounts", MutationAction::Upsert)
        .and_then(|mutation| mutation.with_assignment("id", principal_id.clone()))
        .and_then(|mutation| mutation.with_assignment("email", email.clone()))
        .and_then(|mutation| mutation.with_assignment("username", username.clone()))
        .and_then(|mutation| mutation.with_assignment("display_name", display_name.clone()))
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", synthetic_source_key.clone()))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.on_conflict_field("id"))
        .map_err(import_data_model_error)?;

    Ok((
        mutation,
        serde_json::json!({
            "table": "membership_member_accounts",
            "member_account_id": principal_id,
            "email": email,
            "username": username,
            "display_name": display_name,
            "source_key": synthetic_source_key,
            "bootstrap": "subscription",
            "updated_at": updated_at,
        }),
    ))
}

fn subscription_member_account_bootstrap_statement(
    staged_record: &Value,
) -> Result<(CompiledStatement, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged subscription record is missing `normalized` object data".to_string(),
        })?;
    let principal_id = required_normalized_string(normalized, "principal_id")?;
    MemberAccountId::new(principal_id.clone()).map_err(import_membership_model_error)?;
    let email = optional_normalized_string(normalized, "email")?.unwrap_or_default();
    let username =
        optional_normalized_string(normalized, "username")?.unwrap_or_else(|| principal_id.clone());
    let display_name =
        optional_normalized_string(normalized, "display_name")?.unwrap_or_else(|| username.clone());
    let synthetic_source_key = format!("{source_key}#member-account:{principal_id}");
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate member account bootstrap timestamp: {error}"),
        })?
        .as_secs();

    let insert = MutationSpec::new("membership_member_accounts", MutationAction::Insert)
        .and_then(|mutation| mutation.with_assignment("id", principal_id.clone()))
        .and_then(|mutation| mutation.with_assignment("email", email.clone()))
        .and_then(|mutation| mutation.with_assignment("username", username.clone()))
        .and_then(|mutation| mutation.with_assignment("display_name", display_name.clone()))
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", synthetic_source_key.clone()))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .map_err(import_data_model_error)?
        .compile(1)
        .map_err(import_data_model_error)?;

    Ok((
        CompiledStatement {
            sql: format!("{} ON CONFLICT (\"id\") DO NOTHING", insert.sql),
            bind_values: insert.bind_values,
        },
        serde_json::json!({
            "table": "membership_member_accounts",
            "member_account_id": principal_id,
            "email": email,
            "username": username,
            "display_name": display_name,
            "source_key": synthetic_source_key,
            "bootstrap": "subscription",
            "updated_at": updated_at,
        }),
    ))
}

fn user_import_updates(
    staged_record: &Value,
    site_id: Option<&str>,
    storefront_id: &str,
    auth_package: &dyn AuthModelPackage,
    auth_mapping: &ImportAuthMapping,
) -> Result<(Vec<DefaultTupleUpdate>, Value), ImportModelError> {
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged user record is missing `normalized` object data".to_string(),
        })?;
    let principal_id = required_normalized_string(normalized, "principal_id")?;
    let legacy_roles = normalized
        .get("legacy_roles")
        .and_then(Value::as_array)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged user record is missing `normalized.legacy_roles`".to_string(),
        })?;

    let user = DefaultSubject::entity(Entity::user(principal_id.clone()));
    let mut updates = Vec::new();
    let mut imported_roles = Vec::new();
    let mut effective_roles = Vec::new();
    let mut mapped_capabilities = BTreeMap::<String, Vec<String>>::new();
    let mut granted_scopes = Vec::<Value>::new();
    let mut seen_roles = BTreeSet::new();

    for role in legacy_roles {
        let role = role
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ImportModelError::ManifestParse {
                message: "staged user record has a non-string `normalized.legacy_roles` entry"
                    .to_string(),
            })?;
        if !seen_roles.insert(role.to_string()) {
            continue;
        }
        imported_roles.push(role.to_string());
        let group = Entity::group(format!("legacy-role:{role}"));
        updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
            group.clone(),
            Relation::Member,
            user.clone(),
        )));
        let capability_names = auth_mapping
            .capabilities_for_role(role)
            .ok_or_else(|| ImportModelError::ManifestParse {
                message: format!(
                    "legacy role `{role}` is not declared in the configured import auth mapping document"
                ),
            })?;
        mapped_capabilities.insert(role.to_string(), capability_names.to_vec());
        for grant in derive_import_role_grants(capability_names, auth_package)? {
            let scope_entity = import_auth_scope_entity(grant.scope, site_id, storefront_id)?;
            updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                scope_entity.clone(),
                grant.relation,
                DefaultSubject::userset(group.clone(), Relation::Member),
            )));
            let relation_name = grant.relation.as_str().to_string();
            if !effective_roles
                .iter()
                .any(|candidate| candidate == &relation_name)
            {
                effective_roles.push(relation_name.clone());
            }
            granted_scopes.push(serde_json::json!({
                "legacy_role": role,
                "scope": grant.scope.as_str(),
                "resource_id": scope_entity.id(),
                "relation": relation_name,
                "capabilities": grant.capabilities,
            }));
        }
    }

    Ok((
        updates.clone(),
        serde_json::json!({
            "table": "auth_tuples",
            "principal_id": principal_id,
            "site_id": site_id,
            "storefront_id": storefront_id,
            "legacy_roles": imported_roles,
            "roles": effective_roles,
            "mapped_capabilities": mapped_capabilities,
            "granted_scopes": granted_scopes,
            "writes": updates.len(),
        }),
    ))
}

fn derive_import_role_grants(
    capabilities: &[String],
    auth_package: &dyn AuthModelPackage,
) -> Result<Vec<ResolvedImportAuthGrant>, ImportModelError> {
    let mut grants = Vec::<ResolvedImportAuthGrant>::new();

    for capability_name in capabilities {
        let capability = Capability::from_str(capability_name).ok_or_else(|| {
            ImportModelError::ManifestParse {
                message: format!(
                    "import auth mapping references unsupported capability `{capability_name}`"
                ),
            }
        })?;
        let binding = auth_package
            .binding_for(capability)
            .ok_or_else(|| ImportModelError::ManifestParse {
                message: format!(
                    "auth package `{}` does not bind capability `{}` required by the import auth mapping document",
                    auth_package.manifest().name,
                    capability.as_str()
                ),
            })?;
        let relation = root_role_for_capability_binding(capability, binding.relation)?;
        for namespace in &binding.resource_namespaces {
            if let Some(scope) = import_auth_scope_for_namespace(capability, *namespace)? {
                push_import_role_grant(&mut grants, scope, relation, capability.as_str());
            }
        }
    }

    Ok(grants)
}

fn import_auth_scope_for_namespace(
    capability: Capability,
    namespace: Namespace,
) -> Result<Option<ImportAuthGrantScope>, ImportModelError> {
    match namespace {
        Namespace::Site
        | Namespace::Brand
        | Namespace::Page
        | Namespace::Navigation
        | Namespace::Event
        | Namespace::EventSlot
        | Namespace::Booking
        | Namespace::MediaLibrary
        | Namespace::Media
        | Namespace::AssetFolder
        | Namespace::Asset
        | Namespace::ThemeAssetBundle
        | Namespace::AdminModule => Ok(Some(ImportAuthGrantScope::Site)),
        Namespace::Storefront
        | Namespace::Product
        | Namespace::Collection
        | Namespace::Order
        | Namespace::Subscription
        | Namespace::MembershipTier => Ok(Some(ImportAuthGrantScope::Storefront)),
        Namespace::Tenant => Err(ImportModelError::ManifestParse {
            message: format!(
                "capability `{}` cannot be imported from legacy roles because it targets tenant-scoped auth",
                capability.as_str()
            ),
        }),
        Namespace::Group | Namespace::Team | Namespace::User | Namespace::ServiceAccount => {
            Err(ImportModelError::ManifestParse {
                message: format!(
                    "capability `{}` cannot be imported from legacy roles because it targets unsupported principal namespaces",
                    capability.as_str()
                ),
            })
        }
    }
}

fn root_role_for_capability_binding(
    capability: Capability,
    relation: Relation,
) -> Result<Relation, ImportModelError> {
    match relation {
        Relation::Owner => Ok(Relation::Owner),
        Relation::Admin
        | Relation::Manage
        | Relation::Delete
        | Relation::Unpublish
        | Relation::ManageStorage
        | Relation::Refund
        | Relation::CheckIn => Ok(Relation::Admin),
        Relation::Editor | Relation::Edit | Relation::Publish | Relation::Replace => {
            Ok(Relation::Editor)
        }
        Relation::Support => Ok(Relation::Support),
        Relation::Viewer | Relation::View | Relation::Read => Ok(Relation::Viewer),
        Relation::Member | Relation::Book | Relation::Checkout => Ok(Relation::Member),
        Relation::ReadPublic => Err(ImportModelError::ManifestParse {
            message: format!(
                "capability `{}` resolves to `read_public`, which cannot be granted safely for live legacy auth import without resource-specific tuples",
                capability.as_str()
            ),
        }),
        other => Err(ImportModelError::ManifestParse {
            message: format!(
                "capability `{}` resolves to unsupported relation `{}` for live legacy auth import",
                capability.as_str(),
                other.as_str()
            ),
        }),
    }
}

fn push_import_role_grant(
    grants: &mut Vec<ResolvedImportAuthGrant>,
    scope: ImportAuthGrantScope,
    relation: Relation,
    capability: &str,
) {
    if let Some(existing) = grants
        .iter_mut()
        .find(|grant| grant.scope == scope && import_role_covers(grant.relation, relation))
    {
        if !existing
            .capabilities
            .iter()
            .any(|candidate| candidate == capability)
        {
            existing.capabilities.push(capability.to_string());
        }
        return;
    }

    let mut carried_capabilities = vec![capability.to_string()];
    grants.retain_mut(|grant| {
        if grant.scope == scope && import_role_covers(relation, grant.relation) {
            for existing in &grant.capabilities {
                if !carried_capabilities
                    .iter()
                    .any(|candidate| candidate == existing)
                {
                    carried_capabilities.push(existing.clone());
                }
            }
            false
        } else {
            true
        }
    });

    grants.push(ResolvedImportAuthGrant {
        scope,
        relation,
        capabilities: carried_capabilities,
    });
}

fn import_role_covers(existing: Relation, required: Relation) -> bool {
    matches!(
        (existing, required),
        (Relation::Owner, Relation::Owner)
            | (Relation::Owner, Relation::Admin)
            | (Relation::Owner, Relation::Editor)
            | (Relation::Owner, Relation::Support)
            | (Relation::Owner, Relation::Viewer)
            | (Relation::Owner, Relation::Member)
            | (Relation::Admin, Relation::Admin)
            | (Relation::Admin, Relation::Editor)
            | (Relation::Admin, Relation::Support)
            | (Relation::Admin, Relation::Viewer)
            | (Relation::Admin, Relation::Member)
            | (Relation::Editor, Relation::Editor)
            | (Relation::Editor, Relation::Viewer)
            | (Relation::Editor, Relation::Member)
            | (Relation::Support, Relation::Support)
            | (Relation::Support, Relation::Viewer)
            | (Relation::Support, Relation::Member)
            | (Relation::Viewer, Relation::Viewer)
            | (Relation::Viewer, Relation::Member)
            | (Relation::Member, Relation::Member)
    )
}

fn import_auth_scope_entity(
    scope: ImportAuthGrantScope,
    site_id: Option<&str>,
    storefront_id: &str,
) -> Result<Entity, ImportModelError> {
    match scope {
        ImportAuthGrantScope::Site => {
            let site_id = site_id.ok_or_else(|| ImportModelError::ManifestParse {
                message: "live user import requires a non-empty `site`".to_string(),
            })?;
            if site_id.is_empty() {
                return Err(ImportModelError::ManifestParse {
                    message: "live user import requires a non-empty `site`".to_string(),
                });
            }
            Ok(Entity::site(site_id.to_string()))
        }
        ImportAuthGrantScope::Storefront => Ok(Entity::storefront(storefront_id.to_string())),
    }
}

fn user_account_import_mutation(
    staged_record: &Value,
) -> Result<(MutationSpec, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let target_id = required_staged_string(staged_record, "target_id")?;
    MemberAccountId::new(target_id.clone()).map_err(import_membership_model_error)?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged user record is missing `normalized` object data".to_string(),
        })?;
    let email = optional_normalized_string(normalized, "email")?.unwrap_or_default();
    let username = optional_normalized_string(normalized, "username")?.unwrap_or_default();
    let display_name = optional_normalized_string(normalized, "display_name")?.unwrap_or_default();
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate user account update timestamp: {error}"),
        })?
        .as_secs();

    let mutation = MutationSpec::new("membership_member_accounts", MutationAction::Upsert)
        .and_then(|mutation| mutation.with_assignment("id", target_id.clone()))
        .and_then(|mutation| mutation.with_assignment("email", email.clone()))
        .and_then(|mutation| mutation.with_assignment("username", username.clone()))
        .and_then(|mutation| mutation.with_assignment("display_name", display_name.clone()))
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", source_key))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.on_conflict_field("id"))
        .map_err(import_data_model_error)?;

    Ok((
        mutation,
        serde_json::json!({
            "table": "membership_member_accounts",
            "member_account_id": target_id,
            "email": email,
            "username": username,
            "display_name": display_name,
            "updated_at": updated_at,
        }),
    ))
}

fn page_import_mutation(
    staged_record: &Value,
    default_locale: &str,
) -> Result<(MutationSpec, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let target_id = required_staged_string(staged_record, "target_id")?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged page record is missing `normalized` object data".to_string(),
        })?;
    let locale = normalized
        .get("locale")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_locale)
        .to_string();
    let title = required_normalized_string(normalized, "title")?;
    let slug = required_normalized_string(normalized, "slug")?;
    let template = required_normalized_string(normalized, "template")?;
    let body_html = required_normalized_string(normalized, "body_html")?;
    let workflow_status = required_normalized_string(normalized, "publication_state")?;
    let seo = normalized
        .get("seo")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged page record is missing `normalized.seo` object data".to_string(),
        })?;
    let seo_title = optional_object_string(seo, "title")?.unwrap_or_default();
    let seo_description = optional_object_string(seo, "description")?.unwrap_or_default();
    let canonical_path = optional_object_string(seo, "canonical_path")?.unwrap_or_default();
    let media_references = normalized
        .get("media_references")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let live_path = format!("/{locale}/{slug}");
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate page update timestamp: {error}"),
        })?
        .as_secs();

    let mutation = MutationSpec::new("cms_pages", MutationAction::Upsert)
        .and_then(|mutation| mutation.with_assignment("page_id", target_id.clone()))
        .and_then(|mutation| mutation.with_assignment("locale", locale.clone()))
        .and_then(|mutation| mutation.with_assignment("title", title))
        .and_then(|mutation| mutation.with_assignment("slug", slug))
        .and_then(|mutation| mutation.with_assignment("template", template))
        .and_then(|mutation| mutation.with_assignment("body_html", body_html))
        .and_then(|mutation| mutation.with_assignment("live_path", live_path.clone()))
        .and_then(|mutation| mutation.with_assignment("workflow_status", workflow_status))
        .and_then(|mutation| mutation.with_assignment("seo_title", seo_title))
        .and_then(|mutation| mutation.with_assignment("seo_description", seo_description))
        .and_then(|mutation| mutation.with_assignment("canonical_path", canonical_path))
        .and_then(|mutation| {
            mutation.with_assignment("media_references", media_references.to_string())
        })
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", source_key))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.on_conflict_field("page_id"))
        .map_err(import_data_model_error)?;

    Ok((
        mutation,
        serde_json::json!({
            "table": "cms_pages",
            "page_id": target_id,
            "live_path": live_path,
            "locale": locale,
            "updated_at": updated_at,
        }),
    ))
}

fn event_import_mutation(staged_record: &Value) -> Result<(MutationSpec, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let target_id = required_staged_string(staged_record, "target_id")?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged event record is missing `normalized` object data".to_string(),
        })?;
    let title = required_normalized_string(normalized, "title")?;
    let slug = required_normalized_string(normalized, "slug")?;
    let status = required_normalized_string(normalized, "publication_state")?;
    let starts_at = required_normalized_string(normalized, "starts_at")?;
    let ends_at = optional_normalized_string(normalized, "ends_at")?.unwrap_or_default();
    let summary = optional_normalized_string(normalized, "summary")?.unwrap_or_default();
    let hero_asset = optional_normalized_string(normalized, "hero_asset")?.unwrap_or_default();
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate event update timestamp: {error}"),
        })?
        .as_secs();

    let mutation = MutationSpec::new("events_catalog", MutationAction::Upsert)
        .and_then(|mutation| mutation.with_assignment("id", target_id.clone()))
        .and_then(|mutation| mutation.with_assignment("slug", slug))
        .and_then(|mutation| mutation.with_assignment("title", title))
        .and_then(|mutation| mutation.with_assignment("status", status))
        .and_then(|mutation| mutation.with_assignment("starts_at", starts_at))
        .and_then(|mutation| mutation.with_assignment("ends_at", ends_at))
        .and_then(|mutation| mutation.with_assignment("summary", summary))
        .and_then(|mutation| mutation.with_assignment("hero_asset", hero_asset))
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", source_key))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint))
        .and_then(|mutation| mutation.with_assignment("published_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.on_conflict_field("id"))
        .map_err(import_data_model_error)?;

    Ok((
        mutation,
        serde_json::json!({
            "table": "events_catalog",
            "event_id": target_id,
            "updated_at": updated_at,
        }),
    ))
}

fn membership_tier_import_mutation(
    staged_record: &Value,
) -> Result<(MutationSpec, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let target_id = required_staged_string(staged_record, "target_id")?;
    MembershipTierId::new(target_id.clone()).map_err(import_membership_model_error)?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged membership tier record is missing `normalized` object data"
                .to_string(),
        })?;
    let title = required_normalized_string(normalized, "title")?;
    let entitlement_key =
        EntitlementKey::new(required_normalized_string(normalized, "entitlement_key")?).map_err(
            |error| ImportModelError::ManifestParse {
                message: error.to_string(),
            },
        )?;
    let rank = optional_normalized_u64(normalized, "rank")?.unwrap_or_default();
    let interval = parse_billing_interval(required_normalized_string(normalized, "interval")?)?;
    let grace_period_days =
        optional_normalized_u64(normalized, "grace_period_days")?.unwrap_or_default();
    let visibility = parse_tier_visibility(required_normalized_string(normalized, "visibility")?)?;
    let status = required_normalized_string(normalized, "status")?;
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate membership tier update timestamp: {error}"),
        })?
        .as_secs();

    let mutation = MutationSpec::new("membership_tiers", MutationAction::Upsert)
        .and_then(|mutation| mutation.with_assignment("id", target_id.clone()))
        .and_then(|mutation| mutation.with_assignment("name", title))
        .and_then(|mutation| {
            mutation.with_assignment("entitlement_key", entitlement_key.to_string())
        })
        .and_then(|mutation| mutation.with_assignment("rank", DataValue::UInt(rank)))
        .and_then(|mutation| {
            mutation.with_assignment("interval", render_billing_interval(interval))
        })
        .and_then(|mutation| {
            mutation.with_assignment("grace_period_days", DataValue::UInt(grace_period_days))
        })
        .and_then(|mutation| {
            mutation.with_assignment("visibility", render_tier_visibility(visibility))
        })
        .and_then(|mutation| mutation.with_assignment("status", status.clone()))
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", source_key))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.on_conflict_field("id"))
        .map_err(import_data_model_error)?;

    Ok((
        mutation,
        serde_json::json!({
            "table": "membership_tiers",
            "tier_id": target_id,
            "status": status,
            "updated_at": updated_at,
        }),
    ))
}

fn subscription_import_persistence(
    staged_record: &Value,
    storefront_id: &str,
) -> Result<(Vec<MutationSpec>, Vec<DefaultTupleUpdate>, Value), ImportModelError> {
    let source_system = required_staged_string(staged_record, "source_system")?;
    let source_key = required_staged_string(staged_record, "source_key")?;
    let target_id = required_staged_string(staged_record, "target_id")?;
    SubscriptionId::new(target_id.clone()).map_err(import_membership_model_error)?;
    let batch_id = required_staged_string(staged_record, "checksum")?;
    let fingerprint = required_staged_string(staged_record, "checksum")?;
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged subscription record is missing `normalized` object data".to_string(),
        })?;
    let tier_id = required_normalized_string(normalized, "tier_id")?;
    MembershipTierId::new(tier_id.clone()).map_err(import_membership_model_error)?;
    let principal_id = required_normalized_string(normalized, "principal_id")?;
    MemberAccountId::new(principal_id.clone()).map_err(import_membership_model_error)?;
    let status = parse_subscription_status(required_normalized_string(normalized, "status")?)?;
    let entitlement_key =
        EntitlementKey::new(required_normalized_string(normalized, "entitlement_key")?).map_err(
            |error| ImportModelError::ManifestParse {
                message: error.to_string(),
            },
        )?;
    let entitlement_id = required_normalized_string(normalized, "entitlement_id")?;
    let active = required_normalized_bool(normalized, "active")?;
    let renews_at = required_normalized_u64(normalized, "renews_at")?;
    let grace_period_ends_at = optional_normalized_u64(normalized, "grace_period_ends_at")?;
    if matches!(status, SubscriptionStatus::InGracePeriod) && grace_period_ends_at.is_none() {
        return Err(ImportModelError::ManifestParse {
            message:
                "staged subscription record in grace period requires `normalized.grace_period_ends_at`"
                    .to_string(),
        });
    }
    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to calculate subscription update timestamp: {error}"),
        })?
        .as_secs();

    let subscription_mutation =
        MutationSpec::new("membership_subscriptions", MutationAction::Upsert)
            .and_then(|mutation| mutation.with_assignment("id", target_id.clone()))
            .and_then(|mutation| mutation.with_assignment("member_id", principal_id.clone()))
            .and_then(|mutation| mutation.with_assignment("tier_id", tier_id.clone()))
            .and_then(|mutation| mutation.with_assignment("status", status.to_string()))
            .and_then(|mutation| {
                mutation.with_assignment("entitlement_key", entitlement_key.to_string())
            })
            .and_then(|mutation| mutation.with_assignment("renews_at", DataValue::UInt(renews_at)))
            .and_then(|mutation| {
                mutation.with_assignment(
                    "grace_period_ends_at",
                    DataValue::UInt(grace_period_ends_at.unwrap_or_default()),
                )
            })
            .and_then(|mutation| {
                mutation.with_assignment("cancel_at_period_end", DataValue::Bool(false))
            })
            .and_then(|mutation| mutation.with_assignment("source_system", source_system.clone()))
            .and_then(|mutation| mutation.with_assignment("source_key", source_key.clone()))
            .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id.clone()))
            .and_then(|mutation| mutation.with_assignment("fingerprint", fingerprint.clone()))
            .and_then(|mutation| {
                mutation.with_assignment("updated_at", DataValue::UInt(updated_at))
            })
            .and_then(|mutation| mutation.on_conflict_field("id"))
            .map_err(import_data_model_error)?;
    let entitlement_mutation = MutationSpec::new("membership_entitlements", MutationAction::Upsert)
        .and_then(|mutation| mutation.with_assignment("id", entitlement_id.clone()))
        .and_then(|mutation| mutation.with_assignment("subscription_id", target_id.clone()))
        .and_then(|mutation| {
            mutation.with_assignment("entitlement_key", entitlement_key.to_string())
        })
        .and_then(|mutation| mutation.with_assignment("active", DataValue::Bool(active)))
        .and_then(|mutation| mutation.with_assignment("source_system", source_system))
        .and_then(|mutation| mutation.with_assignment("source_key", source_key))
        .and_then(|mutation| mutation.with_assignment("import_batch_id", batch_id))
        .and_then(|mutation| mutation.with_assignment("updated_at", DataValue::UInt(updated_at)))
        .and_then(|mutation| mutation.on_conflict_field("id"))
        .map_err(import_data_model_error)?;
    let auth_updates = vec![
        DefaultTupleUpdate::Write(DefaultTuple::new(
            Entity::subscription(target_id.clone()),
            Relation::Storefront,
            DefaultSubject::entity(Entity::storefront(storefront_id.to_string())),
        )),
        DefaultTupleUpdate::Write(DefaultTuple::new(
            Entity::subscription(target_id.clone()),
            Relation::Owner,
            DefaultSubject::entity(Entity::user(principal_id.clone())),
        )),
    ];
    let auth_write_count = auth_updates.len();

    Ok((
        vec![subscription_mutation, entitlement_mutation],
        auth_updates,
        serde_json::json!([
            {
                "table": "membership_subscriptions",
                "subscription_id": target_id,
                "tier_id": tier_id,
                "status": status.to_string(),
                "renews_at": renews_at,
                "grace_period_ends_at": grace_period_ends_at,
                "updated_at": updated_at,
            },
            {
                "table": "membership_entitlements",
                "entitlement_id": entitlement_id,
                "subscription_id": target_id,
                "entitlement_key": entitlement_key.to_string(),
                "active": active,
                "updated_at": updated_at,
            },
            {
                "table": "auth_tuples",
                "principal_id": principal_id,
                "subscription_id": target_id,
                "writes": auth_write_count,
            }
        ]),
    ))
}

fn required_staged_string(record: &Value, field: &str) -> Result<String, ImportModelError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: format!("staged import record is missing `{field}`"),
        })
}

fn required_normalized_string(
    record: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, ImportModelError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: format!("staged import record is missing `normalized.{field}`"),
        })
}

fn optional_normalized_string(
    record: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, ImportModelError> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ImportModelError::ManifestParse {
            message: format!("staged import record field `normalized.{field}` must be a string"),
        }),
    }
}

fn optional_normalized_u64(
    record: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, ImportModelError> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value.as_u64().map(Some).ok_or_else(|| {
            ImportModelError::ManifestParse {
                message: format!(
                    "staged import record field `normalized.{field}` must be an unsigned integer"
                ),
            }
        }),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => value.parse::<u64>().map(Some).map_err(|_| {
            ImportModelError::ManifestParse {
                message: format!(
                    "staged import record field `normalized.{field}` must be an unsigned integer"
                ),
            }
        }),
        Some(_) => Err(ImportModelError::ManifestParse {
            message: format!(
                "staged import record field `normalized.{field}` must be an unsigned integer"
            ),
        }),
    }
}

fn required_normalized_u64(
    record: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<u64, ImportModelError> {
    optional_normalized_u64(record, field)?.ok_or_else(|| ImportModelError::ManifestParse {
        message: format!("staged import record is missing `normalized.{field}`"),
    })
}

fn required_normalized_bool(
    record: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<bool, ImportModelError> {
    record
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: format!("staged import record is missing `normalized.{field}`"),
        })
}

fn optional_object_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<Option<String>, ImportModelError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(ImportModelError::ManifestParse {
            message: format!(
                "staged import record field `normalized.seo.{field}` must be a string"
            ),
        }),
    }
}

fn import_data_model_error(error: davenda_data::DataModelError) -> ImportModelError {
    ImportModelError::ManifestParse {
        message: error.to_string(),
    }
}

fn import_membership_model_error(
    error: davenda_memberships::MembershipModelError,
) -> ImportModelError {
    ImportModelError::ManifestParse {
        message: error.to_string(),
    }
}

fn parse_billing_interval(value: String) -> Result<BillingInterval, ImportModelError> {
    match value.as_str() {
        "monthly" => Ok(BillingInterval::Monthly),
        "quarterly" => Ok(BillingInterval::Quarterly),
        "annual" => Ok(BillingInterval::Annual),
        _ => value
            .strip_prefix("custom_days:")
            .and_then(|days| days.parse::<u16>().ok())
            .map(BillingInterval::CustomDays)
            .ok_or_else(|| ImportModelError::ManifestParse {
                message: format!(
                    "membership tier interval `{value}` must be `monthly`, `quarterly`, `annual`, or `custom_days:<days>`"
                ),
            }),
    }
}

fn render_billing_interval(interval: BillingInterval) -> String {
    match interval {
        BillingInterval::Monthly => "monthly".to_string(),
        BillingInterval::Quarterly => "quarterly".to_string(),
        BillingInterval::Annual => "annual".to_string(),
        BillingInterval::CustomDays(days) => format!("custom_days:{days}"),
    }
}

fn parse_tier_visibility(value: String) -> Result<TierVisibility, ImportModelError> {
    match value.as_str() {
        "public" => Ok(TierVisibility::Public),
        "invite_only" => Ok(TierVisibility::InviteOnly),
        "staff_managed" => Ok(TierVisibility::StaffManaged),
        _ => Err(ImportModelError::ManifestParse {
            message: format!(
                "membership tier visibility `{value}` must be `public`, `invite_only`, or `staff_managed`"
            ),
        }),
    }
}

fn render_tier_visibility(visibility: TierVisibility) -> &'static str {
    match visibility {
        TierVisibility::Public => "public",
        TierVisibility::InviteOnly => "invite_only",
        TierVisibility::StaffManaged => "staff_managed",
    }
}

fn parse_subscription_status(value: String) -> Result<SubscriptionStatus, ImportModelError> {
    match value.as_str() {
        "active" => Ok(SubscriptionStatus::Active),
        "in_grace_period" => Ok(SubscriptionStatus::InGracePeriod),
        _ => Err(ImportModelError::ManifestParse {
            message: format!(
                "subscription status `{value}` is not supported by the current live import path"
            ),
        }),
    }
}

fn storage_override_for_import_default(
    asset_storage_default: davenda_import::AssetStorageDefault,
) -> Option<StoragePolicyOverride> {
    let policy = StoragePolicy::from(match asset_storage_default {
        davenda_import::AssetStorageDefault::PublicUpload => StorageClass::PublicUpload,
        davenda_import::AssetStorageDefault::PrivateShared => StorageClass::PrivateShared,
        davenda_import::AssetStorageDefault::LocalOnlySensitive => StorageClass::LocalOnlySensitive,
    });
    Some(StoragePolicyOverride {
        delivery_mode: Some(policy.delivery_mode),
        sync_mode: Some(policy.sync_mode),
        sensitivity: Some(policy.sensitivity),
    })
}

fn render_storage_delivery(delivery: &StorageDeliveryLocation) -> Value {
    match delivery {
        StorageDeliveryLocation::PublicCdn {
            public_url,
            object_key,
        } => serde_json::json!({
            "kind": "public_cdn",
            "public_url": public_url,
            "object_key": object_key,
        }),
        StorageDeliveryLocation::SignedObject {
            object_key,
            signed_url,
            expires_at_unix_seconds,
        } => serde_json::json!({
            "kind": "signed_object",
            "object_key": object_key,
            "signed_url": signed_url,
            "expires_at_unix_seconds": expires_at_unix_seconds,
        }),
        StorageDeliveryLocation::AppProxy { path } => serde_json::json!({
            "kind": "app_proxy",
            "path": path,
        }),
        StorageDeliveryLocation::LocalPath { path } => serde_json::json!({
            "kind": "local_path",
            "path": path.display().to_string(),
        }),
    }
}

fn load_auth_model_test_document(spec_path: &Path) -> Result<AuthModelTestDocument, CliRunError> {
    let input = fs::read_to_string(spec_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to read auth model test spec `{}`: {error}",
            spec_path.display()
        ))
    })?;
    let document: AuthModelTestDocument = toml::from_str(&input).map_err(|error| {
        CliRunError::execution(format!(
            "failed to parse auth model test spec `{}`: {error}",
            spec_path.display()
        ))
    })?;
    if document.cases.is_empty() {
        return Err(CliRunError::execution(format!(
            "auth model test spec `{}` does not define any `[[case]]` entries",
            spec_path.display()
        )));
    }
    Ok(document)
}

fn parse_auth_subject_spec(input: &str, field: &str) -> Result<DefaultSubject, CliRunError> {
    let (left, relation) = match input.split_once('#') {
        Some((left, relation)) => (left, Some(relation)),
        None => (input, None),
    };
    let entity = parse_auth_entity_spec(left, field)?;

    match relation {
        Some(relation) => {
            let relation = Relation::from_str(relation).ok_or_else(|| {
                CliRunError::execution(format!(
                    "auth model test {field} `{input}` uses unknown relation `{relation}`"
                ))
            })?;
            Ok(DefaultSubject::userset(entity, relation))
        }
        None => Ok(DefaultSubject::entity(entity)),
    }
}

fn parse_auth_entity_spec(input: &str, field: &str) -> Result<Entity, CliRunError> {
    let (namespace, id) = input.split_once(':').ok_or_else(|| {
        CliRunError::execution(format!(
            "auth model test {field} `{input}` must use namespace:id syntax"
        ))
    })?;
    if id.trim().is_empty() {
        return Err(CliRunError::execution(format!(
            "auth model test {field} `{input}` must use a non-empty identifier"
        )));
    }

    let entity = match namespace {
        "tenant" => Entity::tenant(id),
        "site" => Entity::site(id),
        "brand" => Entity::brand(id),
        "storefront" => Entity::storefront(id),
        "user" => Entity::user(id),
        "group" => Entity::group(id),
        "team" => Entity::team(id),
        "service_account" => Entity::service_account(id),
        "page" => Entity::page(id),
        "navigation" => Entity::navigation(id),
        "product" => Entity::product(id),
        "collection" => Entity::collection(id),
        "order" => Entity::order(id),
        "subscription" => Entity::subscription(id),
        "membership_tier" => Entity::membership_tier(id),
        "event" => Entity::event(id),
        "event_slot" => Entity::event_slot(id),
        "booking" => Entity::booking(id),
        "media" => Entity::media(id),
        "media_library" => Entity::media_library(id),
        "asset" => Entity::asset(id),
        "asset_folder" => Entity::asset_folder(id),
        "theme_asset_bundle" => Entity::theme_asset_bundle(id),
        "admin_module" => Entity::admin_module(id),
        other => {
            return Err(CliRunError::execution(format!(
                "auth model test {field} `{input}` uses unknown namespace `{other}`"
            )));
        }
    };

    Ok(entity)
}

fn parse_auth_capability_spec(
    input: &str,
    field: &str,
) -> Result<davenda_auth::Capability, CliRunError> {
    davenda_auth::Capability::from_str(input).ok_or_else(|| {
        CliRunError::execution(format!(
            "auth model test {field} `{input}` uses unknown capability `{input}`"
        ))
    })
}

fn render_entity(entity: &Entity) -> String {
    format!("{}:{}", entity.namespace().as_str(), entity.id())
}

fn render_namespace_identifier(namespace: Namespace, id: &str) -> String {
    format!("{}:{id}", namespace.as_str())
}

fn render_subject(subject: &DefaultSubject) -> String {
    match subject {
        DefaultSubject::Entity(entity) => render_entity(entity),
        DefaultSubject::Userset { object, relation } => {
            format!("{}#{}", render_entity(object), relation.as_str())
        }
    }
}

fn release_plan_owner_label(owner: &MigrationPlanOwner) -> String {
    match owner {
        MigrationPlanOwner::Module(module) => format!("module:{module}"),
        MigrationPlanOwner::AuthPackage(package) => format!("auth:{package}"),
        MigrationPlanOwner::CustomerApp(app_id) => format!("customer_app:{app_id}"),
    }
}

fn release_plan_severity_label(severity: ReleaseDoctorSeverity) -> &'static str {
    match severity {
        ReleaseDoctorSeverity::Info => "info",
        ReleaseDoctorSeverity::Warning => "warning",
        ReleaseDoctorSeverity::Blocking => "blocking",
    }
}

fn environment_label(environment: davenda_config::Environment) -> &'static str {
    match environment {
        davenda_config::Environment::Development => "development",
        davenda_config::Environment::Staging => "staging",
        davenda_config::Environment::Production => "production",
    }
}

fn storage_deployment_label(deployment: davenda_config::StorageDeployment) -> &'static str {
    match deployment {
        davenda_config::StorageDeployment::Distributed => "distributed",
        davenda_config::StorageDeployment::SingleNode => "single_node",
    }
}

fn summarize_items(items: impl IntoIterator<Item = String>) -> String {
    let items = items
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if items.is_empty() {
        return "none".to_string();
    }
    const PREVIEW_LIMIT: usize = 4;
    if items.len() <= PREVIEW_LIMIT {
        return items.join(", ");
    }
    format!(
        "{} (+{} more)",
        items[..PREVIEW_LIMIT].join(", "),
        items.len() - PREVIEW_LIMIT
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct ObjectStoreTestServer {
        endpoint: String,
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl ObjectStoreTestServer {
        fn spawn() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let stop = Arc::new(AtomicBool::new(false));
            let store = Arc::new(Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
            let stop_thread = Arc::clone(&stop);
            let store_thread = Arc::clone(&store);
            let handle = thread::spawn(move || {
                loop {
                    if stop_thread.load(Ordering::SeqCst) {
                        break;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let store = Arc::clone(&store_thread);
                            handle_object_store_request(stream, &store);
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("object-store test server failed: {error}"),
                    }
                }
            });

            Self {
                endpoint,
                stop,
                handle: Some(handle),
            }
        }

        fn endpoint(&self) -> &str {
            &self.endpoint
        }
    }

    impl Drop for ObjectStoreTestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    struct LiveProbeTestServer {
        base_url: String,
        stop: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    #[derive(Debug, Clone)]
    struct LiveProbeResponse {
        status_code: u16,
        content_type: &'static str,
        body: Vec<u8>,
        headers: BTreeMap<String, String>,
        authenticated: Option<Box<LiveProbeResponse>>,
    }

    impl LiveProbeResponse {
        fn html(status_code: u16, body: impl Into<String>) -> Self {
            Self {
                status_code,
                content_type: "text/html; charset=utf-8",
                body: body.into().into_bytes(),
                headers: BTreeMap::new(),
                authenticated: None,
            }
        }

        fn binary(status_code: u16, body: Vec<u8>) -> Self {
            Self {
                status_code,
                content_type: "application/octet-stream",
                body,
                headers: BTreeMap::new(),
                authenticated: None,
            }
        }

        fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
            self.headers.insert(name.into(), value.into());
            self
        }

        fn with_authenticated_response(mut self, response: LiveProbeResponse) -> Self {
            self.authenticated = Some(Box::new(response));
            self
        }
    }

    fn cache_probe_response(
        built: &BuiltCustomerAppContext,
        route: &str,
        body: impl Into<String>,
    ) -> LiveProbeResponse {
        let mut response = LiveProbeResponse::html(200, body);
        if let Ok((execution, _)) = resolve_cache_route_execution(built, route) {
            for (name, value) in &execution.cache_plan.headers {
                response = response.with_header(name.clone(), value.clone());
            }
        }
        response
    }

    impl LiveProbeTestServer {
        fn spawn(
            health_status: &'static str,
            readiness_status: &'static str,
            maintenance_enabled: bool,
            routes: BTreeMap<String, u16>,
        ) -> Self {
            let responses = routes
                .into_iter()
                .map(|(route, status_code)| {
                    (
                        route.clone(),
                        LiveProbeResponse::html(
                            status_code,
                            format!("<html><body>{route}</body></html>"),
                        ),
                    )
                })
                .collect();
            Self::spawn_with_responses(
                health_status,
                readiness_status,
                maintenance_enabled,
                responses,
            )
        }

        fn spawn_with_responses(
            health_status: &'static str,
            readiness_status: &'static str,
            maintenance_enabled: bool,
            routes: BTreeMap<String, LiveProbeResponse>,
        ) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let base_url = format!("http://{}", listener.local_addr().unwrap());
            let stop = Arc::new(AtomicBool::new(false));
            let routes = Arc::new(routes);
            let stop_thread = Arc::clone(&stop);
            let routes_thread = Arc::clone(&routes);
            let handle = thread::spawn(move || {
                loop {
                    if stop_thread.load(Ordering::SeqCst) {
                        break;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => handle_live_probe_request(
                            stream,
                            health_status,
                            readiness_status,
                            maintenance_enabled,
                            &routes_thread,
                        ),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("live probe test server failed: {error}"),
                    }
                }
            });

            Self {
                base_url,
                stop,
                handle: Some(handle),
            }
        }

        fn base_url(&self) -> &str {
            &self.base_url
        }
    }

    impl Drop for LiveProbeTestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct CloudflareTestRecord {
        id: String,
        name: String,
        #[serde(rename = "type")]
        record_type: String,
        content: String,
        #[serde(default)]
        proxied: Option<bool>,
    }

    impl CloudflareTestRecord {
        fn cname(
            id: impl Into<String>,
            name: impl Into<String>,
            content: impl Into<String>,
        ) -> Self {
            Self {
                id: id.into(),
                name: name.into(),
                record_type: "CNAME".to_string(),
                content: content.into(),
                proxied: Some(false),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct CloudflareTestLoadBalancer {
        id: String,
        #[serde(default)]
        default_pools: Vec<String>,
    }

    impl CloudflareTestLoadBalancer {
        fn new(id: impl Into<String>, default_pool: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                default_pools: vec![default_pool.into()],
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct CloudflareTestOriginRule {
        id: String,
        origin: String,
    }

    impl CloudflareTestOriginRule {
        fn new(id: impl Into<String>, origin: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                origin: origin.into(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct CloudflareTestRoutingRule {
        id: String,
        service: String,
    }

    impl CloudflareTestRoutingRule {
        fn new(id: impl Into<String>, service: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                service: service.into(),
            }
        }
    }

    struct CloudflareTestServer {
        base_url: String,
        zone_id: String,
        stop: Arc<AtomicBool>,
        records: Arc<Mutex<BTreeMap<String, CloudflareTestRecord>>>,
        load_balancers: Arc<Mutex<BTreeMap<String, CloudflareTestLoadBalancer>>>,
        load_balancer_update_results: Arc<Mutex<BTreeMap<String, CloudflareTestLoadBalancer>>>,
        origin_rules: Arc<Mutex<BTreeMap<String, CloudflareTestOriginRule>>>,
        routing_rules: Arc<Mutex<BTreeMap<String, CloudflareTestRoutingRule>>>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl CloudflareTestServer {
        fn spawn(zone_id: impl Into<String>, records: Vec<CloudflareTestRecord>) -> Self {
            Self::spawn_extended(zone_id, records, Vec::new(), Vec::new(), Vec::new())
        }

        fn spawn_extended(
            zone_id: impl Into<String>,
            records: Vec<CloudflareTestRecord>,
            load_balancers: Vec<CloudflareTestLoadBalancer>,
            origin_rules: Vec<CloudflareTestOriginRule>,
            routing_rules: Vec<CloudflareTestRoutingRule>,
        ) -> Self {
            let zone_id = zone_id.into();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let base_url = format!("http://{}", listener.local_addr().unwrap());
            let stop = Arc::new(AtomicBool::new(false));
            let records = Arc::new(Mutex::new(
                records
                    .into_iter()
                    .map(|record| (record.name.clone(), record))
                    .collect::<BTreeMap<_, _>>(),
            ));
            let load_balancers = Arc::new(Mutex::new(
                load_balancers
                    .into_iter()
                    .map(|resource| (resource.id.clone(), resource))
                    .collect::<BTreeMap<_, _>>(),
            ));
            let load_balancer_update_results = Arc::new(Mutex::new(BTreeMap::new()));
            let origin_rules = Arc::new(Mutex::new(
                origin_rules
                    .into_iter()
                    .map(|resource| (resource.id.clone(), resource))
                    .collect::<BTreeMap<_, _>>(),
            ));
            let routing_rules = Arc::new(Mutex::new(
                routing_rules
                    .into_iter()
                    .map(|resource| (resource.id.clone(), resource))
                    .collect::<BTreeMap<_, _>>(),
            ));
            let stop_thread = Arc::clone(&stop);
            let records_thread = Arc::clone(&records);
            let load_balancers_thread = Arc::clone(&load_balancers);
            let load_balancer_update_results_thread = Arc::clone(&load_balancer_update_results);
            let origin_rules_thread = Arc::clone(&origin_rules);
            let routing_rules_thread = Arc::clone(&routing_rules);
            let zone_id_thread = zone_id.clone();
            let handle = thread::spawn(move || {
                loop {
                    if stop_thread.load(Ordering::SeqCst) {
                        break;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => handle_cloudflare_test_request(
                            stream,
                            &zone_id_thread,
                            &records_thread,
                            &load_balancers_thread,
                            &load_balancer_update_results_thread,
                            &origin_rules_thread,
                            &routing_rules_thread,
                        ),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("cloudflare test server failed: {error}"),
                    }
                }
            });

            Self {
                base_url,
                zone_id,
                stop,
                records,
                load_balancers,
                load_balancer_update_results,
                origin_rules,
                routing_rules,
                handle: Some(handle),
            }
        }

        fn base_url(&self) -> &str {
            &self.base_url
        }

        fn zone_id(&self) -> &str {
            &self.zone_id
        }

        fn record(&self, hostname: &str) -> CloudflareTestRecord {
            self.records.lock().unwrap().get(hostname).cloned().unwrap()
        }

        fn load_balancer(&self, resource_id: &str) -> CloudflareTestLoadBalancer {
            self.load_balancers
                .lock()
                .unwrap()
                .get(resource_id)
                .cloned()
                .unwrap()
        }

        fn set_load_balancer_update_result(&self, resource: CloudflareTestLoadBalancer) {
            self.load_balancer_update_results
                .lock()
                .unwrap()
                .insert(resource.id.clone(), resource);
        }

        fn origin_rule(&self, resource_id: &str) -> CloudflareTestOriginRule {
            self.origin_rules
                .lock()
                .unwrap()
                .get(resource_id)
                .cloned()
                .unwrap()
        }

        fn routing_rule(&self, resource_id: &str) -> CloudflareTestRoutingRule {
            self.routing_rules
                .lock()
                .unwrap()
                .get(resource_id)
                .cloned()
                .unwrap()
        }
    }

    impl Drop for CloudflareTestServer {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    struct DnsCutoverTestContext {
        _lock: std::sync::MutexGuard<'static, ()>,
        server: CloudflareTestServer,
        secret_env_var: String,
        dns_target: String,
    }

    impl Drop for DnsCutoverTestContext {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(CLOUDFLARE_API_BASE_URL_ENV);
                std::env::remove_var(&self.secret_env_var);
            }
        }
    }

    struct TrafficTargetCutoverTestContext {
        _lock: std::sync::MutexGuard<'static, ()>,
        server: CloudflareTestServer,
        secret_env_var: String,
        resource_id: String,
        target: String,
    }

    impl Drop for TrafficTargetCutoverTestContext {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(CLOUDFLARE_API_BASE_URL_ENV);
                std::env::remove_var(&self.secret_env_var);
            }
        }
    }

    fn cloudflare_test_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn database_env_test_lock() -> &'static Mutex<()> {
        static LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CustomerAppAssetFixture {
        config_path: PathBuf,
        _server: ObjectStoreTestServer,
        object_store_env_var: String,
    }

    impl Drop for CustomerAppAssetFixture {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(&self.object_store_env_var);
            }
        }
    }

    fn handle_object_store_request(
        mut stream: std::net::TcpStream,
        store: &Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    ) {
        stream.set_nonblocking(false).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let path = parts
            .next()
            .unwrap_or("/")
            .split('?')
            .next()
            .unwrap_or("/")
            .trim_start_matches('/')
            .trim_start_matches("runtime/")
            .to_string();

        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            let trimmed = header.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
        }

        let mut body = vec![0_u8; content_length];
        if content_length > 0 {
            reader.read_exact(&mut body).unwrap();
        }

        let (status, response_body) = match method {
            "PUT" => {
                store.lock().unwrap().insert(path, body);
                ("200 OK", Vec::new())
            }
            "GET" => match store.lock().unwrap().get(&path).cloned() {
                Some(bytes) => ("200 OK", bytes),
                None => ("404 Not Found", b"not found".to_vec()),
            },
            _ => ("405 Method Not Allowed", b"method not allowed".to_vec()),
        };

        let etag_header = if method == "PUT" {
            "ETag: \"test-etag\"\r\n"
        } else {
            ""
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{etag_header}Connection: close\r\n\r\n",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        if !response_body.is_empty() {
            stream.write_all(&response_body).unwrap();
        }
    }

    fn handle_cloudflare_test_request(
        mut stream: std::net::TcpStream,
        expected_zone_id: &str,
        records: &Arc<Mutex<BTreeMap<String, CloudflareTestRecord>>>,
        load_balancers: &Arc<Mutex<BTreeMap<String, CloudflareTestLoadBalancer>>>,
        load_balancer_update_results: &Arc<Mutex<BTreeMap<String, CloudflareTestLoadBalancer>>>,
        origin_rules: &Arc<Mutex<BTreeMap<String, CloudflareTestOriginRule>>>,
        routing_rules: &Arc<Mutex<BTreeMap<String, CloudflareTestRoutingRule>>>,
    ) {
        stream.set_nonblocking(false).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("");
        let request_target = parts.next().unwrap_or("/");
        let (path, query) = match request_target.split_once('?') {
            Some((path, query)) => (path, Some(query)),
            None => (request_target, None),
        };

        let mut content_length = 0usize;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).unwrap();
            let trimmed = header.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
        }

        let mut body = vec![0_u8; content_length];
        if content_length > 0 {
            reader.read_exact(&mut body).unwrap();
        }

        let response_body = match (method, path) {
            ("GET", get_path) if get_path == format!("/zones/{expected_zone_id}/dns_records") => {
                let hostname = query
                    .and_then(|value| {
                        value.split('&').find_map(|pair| {
                            let (name, value) = pair.split_once('=')?;
                            (name == "name").then_some(value)
                        })
                    })
                    .unwrap_or_default();
                let result = records
                    .lock()
                    .unwrap()
                    .get(hostname)
                    .cloned()
                    .into_iter()
                    .collect::<Vec<_>>();
                serde_json::json!({
                    "success": true,
                    "errors": [],
                    "result": result,
                })
                .to_string()
                .into_bytes()
            }
            ("PUT", put_path)
                if put_path.starts_with(&format!("/zones/{expected_zone_id}/dns_records/")) =>
            {
                let record_id =
                    put_path.trim_start_matches(&format!("/zones/{expected_zone_id}/dns_records/"));
                let update: Value = serde_json::from_slice(&body).unwrap();
                let hostname = update["name"].as_str().unwrap();
                let content = update["content"].as_str().unwrap();
                let proxied = update.get("proxied").and_then(Value::as_bool);
                let mut guard = records.lock().unwrap();
                let record = guard.get_mut(hostname).unwrap();
                assert_eq!(record.id, record_id);
                record.content = content.to_string();
                record.proxied = proxied;
                serde_json::json!({
                    "success": true,
                    "errors": [],
                    "result": record.clone(),
                })
                .to_string()
                .into_bytes()
            }
            ("GET", get_path)
                if get_path.starts_with(&format!("/zones/{expected_zone_id}/load_balancers/")) =>
            {
                let resource_id = get_path
                    .trim_start_matches(&format!("/zones/{expected_zone_id}/load_balancers/"));
                let result = load_balancers.lock().unwrap().get(resource_id).cloned();
                serde_json::json!({
                    "success": result.is_some(),
                    "errors": if result.is_some() { vec![] } else { vec![serde_json::json!({ "message": "unsupported request" })] },
                    "result": result,
                })
                .to_string()
                .into_bytes()
            }
            ("PUT", put_path)
                if put_path.starts_with(&format!("/zones/{expected_zone_id}/load_balancers/")) =>
            {
                let resource_id = put_path
                    .trim_start_matches(&format!("/zones/{expected_zone_id}/load_balancers/"));
                let update: Value = serde_json::from_slice(&body).unwrap();
                let pools = update["default_pools"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|value| value.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>();
                let mut guard = load_balancers.lock().unwrap();
                let resource = guard.get_mut(resource_id).unwrap();
                resource.default_pools = pools;
                let response_resource = load_balancer_update_results
                    .lock()
                    .unwrap()
                    .get(resource_id)
                    .cloned()
                    .unwrap_or_else(|| resource.clone());
                serde_json::json!({
                    "success": true,
                    "errors": [],
                    "result": response_resource,
                })
                .to_string()
                .into_bytes()
            }
            ("GET", get_path)
                if get_path.starts_with(&format!("/zones/{expected_zone_id}/origin_rules/")) =>
            {
                let resource_id = get_path
                    .trim_start_matches(&format!("/zones/{expected_zone_id}/origin_rules/"));
                let result = origin_rules.lock().unwrap().get(resource_id).cloned();
                serde_json::json!({
                    "success": result.is_some(),
                    "errors": if result.is_some() { vec![] } else { vec![serde_json::json!({ "message": "unsupported request" })] },
                    "result": result,
                })
                .to_string()
                .into_bytes()
            }
            ("PUT", put_path)
                if put_path.starts_with(&format!("/zones/{expected_zone_id}/origin_rules/")) =>
            {
                let resource_id = put_path
                    .trim_start_matches(&format!("/zones/{expected_zone_id}/origin_rules/"));
                let update: Value = serde_json::from_slice(&body).unwrap();
                let origin = update["origin"].as_str().unwrap();
                let mut guard = origin_rules.lock().unwrap();
                let resource = guard.get_mut(resource_id).unwrap();
                resource.origin = origin.to_string();
                serde_json::json!({
                    "success": true,
                    "errors": [],
                    "result": resource.clone(),
                })
                .to_string()
                .into_bytes()
            }
            ("GET", get_path)
                if get_path.starts_with(&format!("/zones/{expected_zone_id}/routing_rules/")) =>
            {
                let resource_id = get_path
                    .trim_start_matches(&format!("/zones/{expected_zone_id}/routing_rules/"));
                let result = routing_rules.lock().unwrap().get(resource_id).cloned();
                serde_json::json!({
                    "success": result.is_some(),
                    "errors": if result.is_some() { vec![] } else { vec![serde_json::json!({ "message": "unsupported request" })] },
                    "result": result,
                })
                .to_string()
                .into_bytes()
            }
            ("PUT", put_path)
                if put_path.starts_with(&format!("/zones/{expected_zone_id}/routing_rules/")) =>
            {
                let resource_id = put_path
                    .trim_start_matches(&format!("/zones/{expected_zone_id}/routing_rules/"));
                let update: Value = serde_json::from_slice(&body).unwrap();
                let service = update["service"].as_str().unwrap();
                let mut guard = routing_rules.lock().unwrap();
                let resource = guard.get_mut(resource_id).unwrap();
                resource.service = service.to_string();
                serde_json::json!({
                    "success": true,
                    "errors": [],
                    "result": resource.clone(),
                })
                .to_string()
                .into_bytes()
            }
            _ => serde_json::json!({
                "success": false,
                "errors": [{ "message": "unsupported request" }],
                "result": Value::Null,
            })
            .to_string()
            .into_bytes(),
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        stream.write_all(&response_body).unwrap();
    }

    fn handle_live_probe_request(
        mut stream: std::net::TcpStream,
        health_status: &str,
        readiness_status: &str,
        maintenance_enabled: bool,
        routes: &BTreeMap<String, LiveProbeResponse>,
    ) {
        stream.set_nonblocking(false).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();
        let mut cookie_header = None;
        loop {
            let mut header_line = String::new();
            reader.read_line(&mut header_line).unwrap();
            let trimmed = header_line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some((name, value)) = trimmed.split_once(':') {
                if name.eq_ignore_ascii_case("cookie") {
                    cookie_header = Some(value.trim().to_string());
                }
            }
        }
        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .split('?')
            .next()
            .unwrap_or("/");
        let authenticated_request = cookie_header
            .as_deref()
            .is_some_and(|cookie| !cookie.trim().is_empty());

        let (status, content_type, response_body, response_headers) = match path {
            "/health" => (
                "200 OK".to_string(),
                "application/json",
                serde_json::json!({
                    "status": health_status,
                    "maintenance": { "enabled": maintenance_enabled }
                })
                .to_string()
                .into_bytes(),
                BTreeMap::<String, String>::new(),
            ),
            "/ready" | "/readiness" => (
                "200 OK".to_string(),
                "application/json",
                serde_json::json!({ "status": readiness_status })
                    .to_string()
                    .into_bytes(),
                BTreeMap::<String, String>::new(),
            ),
            route => {
                let response = routes.get(route).cloned().unwrap_or_else(|| {
                    LiveProbeResponse::html(404, format!("<html><body>{route}</body></html>"))
                });
                let authenticated_response = response.authenticated.as_deref().cloned();
                let response = if authenticated_request {
                    authenticated_response.unwrap_or(response)
                } else {
                    response
                };
                let status = reqwest::StatusCode::from_u16(response.status_code)
                    .ok()
                    .and_then(|status| {
                        status
                            .canonical_reason()
                            .map(|reason| format!("{} {}", response.status_code, reason))
                    })
                    .unwrap_or_else(|| format!("{} Unknown", response.status_code));
                (
                    status.to_string(),
                    response.content_type,
                    response.body,
                    response.headers,
                )
            }
        };

        let extra_headers = response_headers
            .into_iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
        if !response_body.is_empty() {
            stream.write_all(&response_body).unwrap();
        }
    }

    const DISABLED_EXPLAIN_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
deployment = "single_node"
single_node_escape_hatch = "explicit_single_node"
local_root = "/tmp/davenda-cli"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en"
supported_locales = ["en"]
fallback_locale = "en"
localized_routes = false

[seo]
canonical_host = "example.com"
emit_json_ld = true
sitemap_enabled = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms"]

[wasm]
directory = "wasm"
default_time_limit_ms = 1000
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#;

    const CUSTOMER_APP_MANIFEST: &str = r#"
[app]
name = "showcase-events"
display_name = "Showcase Events"

[domains]
canonical = "example.com"

[i18n]
default_locale = "en"
supported_locales = ["en"]

[theme]
active = "storefront"
template_namespaces = ["pages", "layouts"]

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms"]

[[customer_migrations]]
id = "site-navigation"
order = 10
description = "Migrate the customer navigation structure"
"#;

    const CUSTOMER_APP_MANIFEST_WITH_ASSETS: &str = r#"
[app]
name = "showcase-events"
display_name = "Showcase Events"

[domains]
canonical = "example.com"

[i18n]
default_locale = "en"
supported_locales = ["en"]

[theme]
active = "storefront"
template_namespaces = ["pages", "layouts"]
asset_roots = ["theme/assets"]

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms"]
"#;

    fn customer_app_fixture() -> PathBuf {
        customer_app_fixture_with_modules(&["cms"])
    }

    fn customer_app_fixture_with_tls_config(tls_config: &str) -> PathBuf {
        customer_app_fixture_with_rendered_config(
            DISABLED_EXPLAIN_CONFIG.replace("[tls]\nmode = \"external\"", tls_config),
            &["cms"],
        )
    }

    fn customer_app_fixture_with_modules(modules: &[&str]) -> PathBuf {
        customer_app_fixture_with_rendered_config(DISABLED_EXPLAIN_CONFIG.to_string(), modules)
    }

    fn customer_app_fixture_with_rendered_config(
        config_contents: String,
        modules: &[&str],
    ) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("davenda-cli-workflow-{suffix}"));
        let config_dir = root.join("config");
        let app_root = root.join("apps").join("showcase-events");
        let auth_root = app_root.join("auth").join("shoppr-auth");
        let templates_root = app_root.join("templates").join("pages");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&auth_root).unwrap();
        fs::create_dir_all(&templates_root).unwrap();
        fs::write(
            config_dir.join("platform.toml"),
            render_fixture_modules(&config_contents, modules),
        )
        .unwrap();
        fs::write(
            app_root.join("app.toml"),
            render_fixture_modules(CUSTOMER_APP_MANIFEST, modules),
        )
        .unwrap();
        fs::write(
            templates_root.join("home.html"),
            "<html><body><main>Showcase Events</main></body></html>",
        )
        .unwrap();
        fs::write(
            auth_root.join("package.toml"),
            "name = \"shoppr-auth\"\nversion = \"0.1.0\"\nmode = \"extend\"\nstorage_schema_version = 1\nmodel_version = 1\ncapability_binding_version = 1\nimports = [\"platform-default-auth\"]\n",
        )
        .unwrap();
        fs::write(
            auth_root.join("model.auth"),
            "type product\n  relations\n    merchandiser: user | group#member\n  permissions\n    featured_edit = merchandiser\n",
        )
        .unwrap();
        fs::write(
            auth_root.join("capabilities.toml"),
            "[bindings.\"catalog.featured.edit\"]\nresource_type = \"product\"\npermission = \"featured_edit\"\n",
        )
        .unwrap();

        config_dir.join("platform.toml")
    }

    fn render_fixture_modules(input: &str, modules: &[&str]) -> String {
        let rendered = format!(
            "enabled = [{}]",
            modules
                .iter()
                .map(|module| format!("\"{module}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        input.replace("enabled = [\"cms\"]", &rendered)
    }

    struct ImportFixture {
        manifest_path: PathBuf,
        journal_path: PathBuf,
        root: PathBuf,
        _server: ObjectStoreTestServer,
        object_store_env_var: String,
    }

    impl Drop for ImportFixture {
        fn drop(&mut self) {
            unsafe {
                std::env::remove_var(&self.object_store_env_var);
            }
        }
    }

    fn configure_dns_cutover_for_import_fixture(_fixture: &ImportFixture) -> DnsCutoverTestContext {
        let lock = cloudflare_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dns_target = "davenda-origin.example.net".to_string();
        let server = CloudflareTestServer::spawn(
            format!("zone-{suffix}"),
            vec![CloudflareTestRecord::cname(
                format!("record-{suffix}"),
                "shop.example.com",
                "legacy-origin.example.net",
            )],
        );
        unsafe {
            std::env::set_var(
                CUTOVER_CLOUDFLARE_SECRET_ENV,
                r#"{"cloudflare_api_token":"test-cloudflare-token"}"#,
            );
            std::env::set_var(CLOUDFLARE_API_BASE_URL_ENV, server.base_url());
        }

        DnsCutoverTestContext {
            _lock: lock,
            server,
            secret_env_var: CUTOVER_CLOUDFLARE_SECRET_ENV.to_string(),
            dns_target,
        }
    }

    fn configure_load_balancer_cutover_for_import_fixture(
        _fixture: &ImportFixture,
    ) -> TrafficTargetCutoverTestContext {
        configure_traffic_target_cutover(
            "load-balancer",
            vec![CloudflareTestLoadBalancer::new(
                "lb-edge-1",
                "legacy-origin-pool",
            )],
            Vec::new(),
            Vec::new(),
            "lb-edge-1",
            "davenda-origin-pool",
        )
    }

    fn configure_cdn_origin_cutover_for_import_fixture(
        _fixture: &ImportFixture,
    ) -> TrafficTargetCutoverTestContext {
        configure_traffic_target_cutover(
            "cdn-origin",
            Vec::new(),
            vec![CloudflareTestOriginRule::new(
                "origin-main",
                "legacy-origin.example.net",
            )],
            Vec::new(),
            "origin-main",
            "davenda-origin.example.net",
        )
    }

    fn configure_routing_cutover_for_import_fixture(
        _fixture: &ImportFixture,
    ) -> TrafficTargetCutoverTestContext {
        configure_traffic_target_cutover(
            "routing",
            Vec::new(),
            Vec::new(),
            vec![CloudflareTestRoutingRule::new(
                "route-primary",
                "legacy-service",
            )],
            "route-primary",
            "davenda-service",
        )
    }

    fn configure_traffic_target_cutover(
        _label: &str,
        load_balancers: Vec<CloudflareTestLoadBalancer>,
        origin_rules: Vec<CloudflareTestOriginRule>,
        routing_rules: Vec<CloudflareTestRoutingRule>,
        resource_id: &str,
        target: &str,
    ) -> TrafficTargetCutoverTestContext {
        let lock = cloudflare_test_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let server = CloudflareTestServer::spawn_extended(
            format!("zone-{suffix}"),
            vec![CloudflareTestRecord::cname(
                format!("record-{suffix}"),
                "shop.example.com",
                "legacy-origin.example.net",
            )],
            load_balancers,
            origin_rules,
            routing_rules,
        );
        unsafe {
            std::env::set_var(
                CUTOVER_CLOUDFLARE_SECRET_ENV,
                r#"{"cloudflare_api_token":"test-cloudflare-token"}"#,
            );
            std::env::set_var(CLOUDFLARE_API_BASE_URL_ENV, server.base_url());
        }

        TrafficTargetCutoverTestContext {
            _lock: lock,
            server,
            secret_env_var: CUTOVER_CLOUDFLARE_SECRET_ENV.to_string(),
            resource_id: resource_id.to_string(),
            target: target.to_string(),
        }
    }

    fn write_test_file(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    fn ensure_test_tls_material_key() {
        unsafe {
            std::env::set_var(
                "DAVENDA_TLS_MATERIAL_KEY",
                "davenda-test-tls-material-key-seed",
            );
        }
    }

    fn import_fixture() -> ImportFixture {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("davenda-cli-import-{suffix}"));
        let config_dir = root.join("config");
        let app_root = root.join("apps").join("showcase-events");
        let templates_root = app_root.join("templates").join("pages");
        let storage_root = root.join("storage");
        let object_store_server = ObjectStoreTestServer::spawn();
        let object_store_env_var = format!("DAVENDA_IMPORT_OBJECT_STORE_URL_{suffix}");
        let manifest_path = root.join("imports").join("wordpress-events.toml");
        let journal_path = import_journal_path(
            &manifest_path,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&templates_root).unwrap();
        let config = DISABLED_EXPLAIN_CONFIG
            .replace("environment = \"production\"", "environment = \"development\"")
            .replace("canonical_host = \"example.com\"", "canonical_host = \"shop.example.com\"")
            .replace("enabled = [\"cms\"]", "enabled = [\"cms\", \"media\", \"events\"]")
            .replace(
                "[assets]\npublish_manifest = false",
                "[assets]\npublish_manifest = false\ncdn_base_url = \"https://cdn.example.com/assets\"",
            )
            .replace(
                "single_node_escape_hatch = \"explicit_single_node\"\nlocal_root = \"/tmp/davenda-cli\"",
                &format!(
                    "single_node_escape_hatch = \"explicit_single_node\"\nlocal_root = \"{}\"\nobject_store = \"s3\"\nobject_store_secret = {{ kind = \"env\", var = \"{}\" }}",
                    storage_root.display(),
                    object_store_env_var
                ),
            );
        unsafe {
            std::env::set_var(
                &object_store_env_var,
                format!(
                    "bucket = \"runtime\"\nregion = \"us-east-1\"\nendpoint_url = \"{}\"\naccess_key_id = \"runtime-access\"\nsecret_access_key = \"runtime-secret\"\nallow_http = true",
                    object_store_server.endpoint()
                ),
            );
        }
        fs::write(config_dir.join("platform.toml"), config).unwrap();
        fs::write(
            app_root.join("app.toml"),
            r#"
[app]
name = "showcase-events"
display_name = "Showcase Events"

[domains]
canonical = "shop.example.com"

[i18n]
default_locale = "en"
supported_locales = ["en"]

[theme]
active = "storefront"
template_namespaces = ["pages", "layouts"]

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms", "media", "events"]
"#,
        )
        .unwrap();
        fs::write(
            templates_root.join("home.html"),
            "<html><body><main>Showcase Events</main></body></html>",
        )
        .unwrap();

        write_test_file(
            root.join("imports").join("fixtures").join("users.json"),
            r#"[
  {
    "source_key": "wp:user:alice",
    "checksum": "user-alice-v1",
    "principal_id": "alice",
    "email": "alice@example.com",
    "username": "alice",
    "legacy_roles": ["administrator"]
  }
]"#,
        );
        write_test_file(
            root.join("imports").join("fixtures").join("media.json"),
            r#"[
  {
    "source_key": "wp:media:hero",
    "checksum": "media-hero-v1",
    "title": "Hero",
    "slug": "hero",
    "content_type": "image/jpeg",
    "source_url": "https://legacy.example.com/uploads/hero.jpg",
    "source_file": "fixtures/source/uploads/hero.jpg"
  }
]"#,
        );
        write_test_file(
            root.join("imports")
                .join("fixtures")
                .join("source")
                .join("uploads")
                .join("hero.jpg"),
            "fake-jpeg-bytes",
        );
        write_test_file(
            root.join("imports").join("fixtures").join("pages.json"),
            r#"[
  {
    "source_key": "wp:post:home",
    "checksum": "page-home-v1",
    "title": "Home",
    "slug": "home",
    "body_html": "<p>Home</p>",
    "media_references": ["wp:media:hero"]
  }
]"#,
        );
        write_test_file(
            root.join("imports").join("fixtures").join("events.json"),
            r#"[
  {
    "source_key": "wp:event:festival",
    "checksum": "event-festival-v1",
    "title": "Festival",
    "slug": "festival",
    "starts_at": "2026-06-01T10:00:00Z",
    "hero_asset_source_key": "wp:media:hero"
  }
]"#,
        );
        write_test_file(
            &manifest_path,
            r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "showcase-events"
modules = ["cms", "events", "media"]

[target]
app_manifest = "../apps/showcase-events/app.toml"
platform_config = "../config/platform.toml"
expected_modules = ["cms", "media", "events"]

[[importers]]
id = "users"
phase = 10
resource_kind = "user"
description = "Import users"
source_path = "fixtures/users.json"

[[importers]]
id = "media"
phase = 20
resource_kind = "asset"
description = "Import media"
source_path = "fixtures/media.json"

[[importers]]
id = "pages"
phase = 30
resource_kind = "page"
description = "Import pages"
source_path = "fixtures/pages.json"
mapping = { template = "pages/home", page_type = "home" }
dependencies = ["media"]

[[importers]]
id = "events"
phase = 40
resource_kind = "event"
description = "Import events"
source_path = "fixtures/events.json"
dependencies = ["users", "media"]
"#,
        );

        ImportFixture {
            manifest_path,
            journal_path,
            root,
            _server: object_store_server,
            object_store_env_var,
        }
    }

    fn write_cutover_observe_manifest(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
    ) -> PathBuf {
        write_cutover_observe_manifest_for_switch_method(
            fixture,
            name,
            observation_window_minutes,
            "dns",
            &["record_counts"],
        )
    }

    fn write_cutover_observe_manifest_with_routes_and_checks(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        switch_method: &str,
        required_checks: &[&str],
        sample_routes: &[&str],
    ) -> PathBuf {
        let manifest_path = fixture.root.join("imports").join(name);
        let required = required_checks
            .iter()
            .map(|check| format!("\"{check}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let sample_routes = sample_routes
            .iter()
            .map(|route| format!("\"{route}\""))
            .collect::<Vec<_>>()
            .join(", ");
        write_test_file(
            &manifest_path,
            &format!(
                r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "showcase-events"
modules = ["media"]
publication_mode = "publish_validated"
asset_storage_default = "public_upload"

[target]
app_manifest = "../apps/showcase-events/app.toml"
platform_config = "../config/platform.toml"
expected_modules = ["media"]

[verification]
required = [{required}]
sample_routes = [{sample_routes}]

[cutover]
freeze_legacy_writes = false
switch_method = "{switch_method}"
hostnames = ["shop.example.com"]
requires_assets_publish = false
requires_migrate_apply = false
requires_storage_validation = false
requires_cache_warm = false
observation_window_minutes = {observation_window_minutes}

[[cutover.rollback_triggers]]
id = "auth-failure"
description = "Auth failure"

[[importers]]
id = "media"
phase = 20
resource_kind = "asset"
description = "Import media"
source_path = "fixtures/media.json"
"#
            ),
        );
        manifest_path
    }

    fn write_cutover_observe_manifest_with_checks(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        required_checks: &[&str],
    ) -> PathBuf {
        write_cutover_observe_manifest_for_switch_method(
            fixture,
            name,
            observation_window_minutes,
            "dns",
            required_checks,
        )
    }

    fn write_cutover_observe_manifest_for_switch_method(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        switch_method: &str,
        required_checks: &[&str],
    ) -> PathBuf {
        write_cutover_observe_manifest_with_routes_and_checks(
            fixture,
            name,
            observation_window_minutes,
            switch_method,
            required_checks,
            &["/", "/events"],
        )
    }

    fn write_cutover_observe_manifest_with_users_and_checks(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        required_checks: &[&str],
        sample_users: &[&str],
    ) -> PathBuf {
        write_cutover_observe_manifest_with_users_routes_and_checks(
            fixture,
            name,
            observation_window_minutes,
            required_checks,
            &["/", "/events"],
            sample_users,
        )
    }

    fn write_cutover_observe_manifest_with_users_routes_and_checks(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        required_checks: &[&str],
        sample_routes: &[&str],
        sample_users: &[&str],
    ) -> PathBuf {
        let manifest_path = fixture.root.join("imports").join(name);
        let required = required_checks
            .iter()
            .map(|check| format!("\"{check}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let sample_routes = sample_routes
            .iter()
            .map(|route| format!("\"{route}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let sample_users = sample_users
            .iter()
            .map(|user| format!("\"{user}\""))
            .collect::<Vec<_>>()
            .join(", ");
        write_test_file(
            &manifest_path,
            &format!(
                r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "showcase-events"
modules = ["media"]
publication_mode = "publish_validated"
asset_storage_default = "public_upload"

[target]
app_manifest = "../apps/showcase-events/app.toml"
platform_config = "../config/platform.toml"
expected_modules = ["media"]

[verification]
required = [{required}]
sample_routes = [{sample_routes}]
sample_users = [{sample_users}]

[cutover]
freeze_legacy_writes = false
switch_method = "dns"
hostnames = ["shop.example.com"]
requires_assets_publish = false
requires_migrate_apply = false
requires_storage_validation = false
requires_cache_warm = false
observation_window_minutes = {observation_window_minutes}

[[cutover.rollback_triggers]]
id = "auth-failure"
description = "Auth failure"

[[importers]]
id = "media"
phase = 20
resource_kind = "asset"
description = "Import media"
source_path = "fixtures/media.json"
"#
            ),
        );
        manifest_path
    }

    fn write_cutover_observe_manifest_with_webhooks_and_checks(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        required_checks: &[&str],
        webhooks: &[(&str, &str, u32, u32)],
    ) -> PathBuf {
        let manifest_path = fixture.root.join("imports").join(name);
        let required = required_checks
            .iter()
            .map(|check| format!("\"{check}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let webhook_blocks = webhooks
            .iter()
            .map(|(source, event, max_verification_failures, max_replay_rejections)| {
                format!(
                    "\n[[verification.webhooks]]\nsource = \"{source}\"\nevent = \"{event}\"\nmax_verification_failures = {max_verification_failures}\nmax_replay_rejections = {max_replay_rejections}\n"
                )
            })
            .collect::<Vec<_>>()
            .join("");
        write_test_file(
            &manifest_path,
            &format!(
                r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "showcase-events"
modules = ["media"]
publication_mode = "publish_validated"
asset_storage_default = "public_upload"

[target]
app_manifest = "../apps/showcase-events/app.toml"
platform_config = "../config/platform.toml"
expected_modules = ["media"]

[verification]
required = [{required}]
{webhook_blocks}

[cutover]
freeze_legacy_writes = false
switch_method = "dns"
hostnames = ["shop.example.com"]
requires_assets_publish = false
requires_migrate_apply = false
requires_storage_validation = false
requires_cache_warm = false
observation_window_minutes = {observation_window_minutes}

[[cutover.rollback_triggers]]
id = "webhook-failure"
description = "Webhook failure"

[[importers]]
id = "media"
phase = 20
resource_kind = "asset"
description = "Import media"
source_path = "fixtures/media.json"
"#
            ),
        );
        manifest_path
    }

    fn write_cutover_observe_manifest_with_sample_routes(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        required_checks: &[&str],
        sample_routes: &[&str],
    ) -> PathBuf {
        let manifest_path = fixture.root.join("imports").join(name);
        let required = required_checks
            .iter()
            .map(|check| format!("\"{check}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let sample_routes = sample_routes
            .iter()
            .map(|route| format!("\"{route}\""))
            .collect::<Vec<_>>()
            .join(", ");
        write_test_file(
            &manifest_path,
            &format!(
                r#"
run_id = "wordpress-events"
source_system = "wordpress"
snapshot_at = "2026-03-19T00:00:00Z"
customer_app_id = "showcase-events"
modules = ["media"]
publication_mode = "publish_validated"
asset_storage_default = "public_upload"

[target]
app_manifest = "../apps/showcase-events/app.toml"
platform_config = "../config/platform.toml"
expected_modules = ["media"]

[verification]
required = [{required}]
sample_routes = [{sample_routes}]

[cutover]
freeze_legacy_writes = false
switch_method = "dns"
hostnames = ["shop.example.com"]
requires_assets_publish = false
requires_migrate_apply = false
requires_storage_validation = false
requires_cache_warm = false
observation_window_minutes = {observation_window_minutes}

[[cutover.rollback_triggers]]
id = "auth-failure"
description = "Auth failure"

[[importers]]
id = "media"
phase = 20
resource_kind = "asset"
description = "Import media"
source_path = "fixtures/media.json"
"#
            ),
        );
        manifest_path
    }

    fn enable_admin_and_ops_for_import_fixture(fixture: &ImportFixture) {
        let config_path = fixture.root.join("config").join("platform.toml");
        let app_manifest_path = fixture
            .root
            .join("apps")
            .join("showcase-events")
            .join("app.toml");
        let config = fs::read_to_string(&config_path).unwrap().replace(
            "enabled = [\"cms\", \"media\", \"events\"]",
            "enabled = [\"cms\", \"media\", \"events\", \"admin\", \"ops\"]",
        );
        fs::write(&config_path, config).unwrap();
        let app_manifest = fs::read_to_string(&app_manifest_path).unwrap().replace(
            "enabled = [\"cms\", \"media\", \"events\"]",
            "enabled = [\"cms\", \"media\", \"events\", \"admin\", \"ops\"]",
        );
        fs::write(&app_manifest_path, app_manifest).unwrap();
    }

    fn run_dns_cutover_switch(
        manifest_path: &Path,
        base_url: &str,
        dns: &DnsCutoverTestContext,
    ) -> String {
        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            manifest_path.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            base_url.to_string(),
            "--dns-zone-id".to_string(),
            dns.server.zone_id().to_string(),
            "--dns-target".to_string(),
            dns.dns_target.clone(),
            "--yes".to_string(),
        ])
        .unwrap()
    }

    fn run_traffic_target_cutover_switch(
        manifest_path: &Path,
        base_url: &str,
        context: &TrafficTargetCutoverTestContext,
    ) -> String {
        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            manifest_path.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            base_url.to_string(),
            "--switch-zone-id".to_string(),
            context.server.zone_id().to_string(),
            "--switch-resource-id".to_string(),
            context.resource_id.clone(),
            "--switch-target".to_string(),
            context.target.clone(),
            "--yes".to_string(),
        ])
        .unwrap()
    }

    fn choose_cache_probe_route(
        built: &BuiltCustomerAppContext,
        candidates: &[&str],
    ) -> Option<String> {
        for route in candidates {
            if resolve_cache_route_execution(&built, route).is_ok() {
                return Some((*route).to_string());
            }
        }
        None
    }

    fn run_traffic_target_cutover_switch_dry_run(
        manifest_path: &Path,
        base_url: &str,
        context: &TrafficTargetCutoverTestContext,
    ) -> String {
        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            manifest_path.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            base_url.to_string(),
            "--switch-zone-id".to_string(),
            context.server.zone_id().to_string(),
            "--switch-resource-id".to_string(),
            context.resource_id.clone(),
            "--switch-target".to_string(),
            context.target.clone(),
            "--dry-run".to_string(),
        ])
        .unwrap()
    }

    fn customer_app_fixture_with_assets(publish_manifest: bool) -> CustomerAppAssetFixture {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("davenda-cli-assets-{suffix}"));
        let config_dir = root.join("config");
        let app_root = root.join("apps").join("showcase-events");
        let templates_root = app_root.join("templates").join("pages");
        let asset_root = app_root.join("theme").join("assets");
        let local_root = root.join("storage");
        let object_store_server = ObjectStoreTestServer::spawn();
        let object_store_env_var = format!("DAVENDA_OBJECT_STORE_URL_{suffix}");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&templates_root).unwrap();
        fs::create_dir_all(&asset_root).unwrap();
        let mut config =
            DISABLED_EXPLAIN_CONFIG.replace("/tmp/davenda-cli", &local_root.display().to_string());
        config = config.replace(
            "environment = \"production\"",
            "environment = \"development\"",
        );
        config = config.replace(
            &format!(
                "single_node_escape_hatch = \"explicit_single_node\"\nlocal_root = \"{}\"",
                local_root.display()
            ),
            &format!(
                "single_node_escape_hatch = \"explicit_single_node\"\nlocal_root = \"{}\"\nobject_store = \"s3\"\nobject_store_secret = {{ kind = \"env\", var = \"{}\" }}",
                local_root.display(),
                object_store_env_var
            ),
        );
        if publish_manifest {
            config = config.replace(
                "[assets]\npublish_manifest = false",
                "[assets]\npublish_manifest = true\ncdn_base_url = \"https://cdn.example.com/assets\"",
            );
        } else {
            config = config.replace(
                "[assets]\npublish_manifest = false",
                "[assets]\npublish_manifest = false\ncdn_base_url = \"https://cdn.example.com/assets\"",
            );
        }
        fs::write(config_dir.join("platform.toml"), config).unwrap();
        fs::write(app_root.join("app.toml"), CUSTOMER_APP_MANIFEST_WITH_ASSETS).unwrap();
        fs::write(
            templates_root.join("home.html"),
            "<html><body><main>Showcase Events</main></body></html>",
        )
        .unwrap();
        fs::write(asset_root.join("site.css"), "body { color: #123456; }\n").unwrap();
        fs::write(
            asset_root.join("app.js"),
            "window.davenda = { ready: true };\n",
        )
        .unwrap();
        unsafe {
            std::env::set_var(
                &object_store_env_var,
                format!(
                    "bucket = \"runtime\"\nregion = \"us-east-1\"\nendpoint_url = \"{}\"\naccess_key_id = \"runtime-access\"\nsecret_access_key = \"runtime-secret\"\nallow_http = true",
                    object_store_server.endpoint()
                ),
            );
        }

        CustomerAppAssetFixture {
            config_path: config_dir.join("platform.toml"),
            _server: object_store_server,
            object_store_env_var,
        }
    }

    fn harbor_shop_platform_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/shoppr/platform.toml")
            .canonicalize()
            .expect("sample customer app config exists")
    }

    #[test]
    fn run_from_args_returns_usage_for_help() {
        let rendered = run_from_args(["--help".to_string()]).unwrap();
        assert!(rendered.contains("platform config validate [--config <path>]"));
        assert!(rendered.contains("platform auth check [--config <path>]"));
        assert!(rendered.contains("platform auth bindings inspect [--config <path>]"));
        assert!(rendered.contains("platform auth test-model <spec-path> [--config <path>]"));
        assert!(rendered.contains("platform auth list [--config <path>]"));
        assert!(rendered.contains("platform auth lookup [--config <path>]"));
        assert!(rendered.contains("platform auth explain [--config <path>]"));
        assert!(rendered.contains("platform auth package inspect [--config <path>]"));
        assert!(rendered.contains("platform module list [--config <path>]"));
        assert!(rendered.contains("platform module inspect <module> [--config <path>]"));
        assert!(
            rendered
                .contains("platform module install <module> [--config <path>] [--dry-run] [--yes]")
        );
        assert!(
            rendered
                .contains("platform module enable <module> [--config <path>] [--dry-run] [--yes]")
        );
        assert!(
            rendered
                .contains("platform module disable <module> [--config <path>] [--dry-run] [--yes]")
        );
        assert!(rendered.contains("platform migrate plan [--config <path>]"));
        assert!(rendered.contains("platform migrate apply [--config <path>] [--dry-run] [--yes]"));
        assert!(rendered.contains("platform release doctor [--config <path>]"));
        assert!(rendered.contains("platform release plan [--config <path>]"));
        assert!(
            rendered
                .contains("platform cache warm [--config <path>] --scope public --route <path>")
        );
        assert!(rendered.contains("platform jobs status [--config <path>] [--queue <name>]"));
        assert!(rendered.contains(
            "platform jobs run [--config <path>] [--queue <name>] [--worker-id <id>] [--limit <n>] [--dry-run]"
        ));
        assert!(
            rendered
                .contains("platform jobs ready [--config <path>] [--queue <name>] [--limit <n>]")
        );
        assert!(rendered.contains(
            "platform jobs dead-letters [--config <path>] [--queue <name>] [--limit <n>]"
        ));
        assert!(rendered.contains(
            "platform jobs in-flight [--config <path>] [--queue <name>] [--worker-id <id>] [--limit <n>]"
        ));
        assert!(rendered.contains(
            "platform jobs retry <dead-letter-id> [--config <path>] [--dry-run] [--yes]"
        ));
        assert!(rendered.contains("platform jobs promote [--config <path>] [--dry-run] [--yes]"));
        assert!(rendered.contains("platform storage inspect [--config <path>]"));
        assert!(rendered.contains("platform storage verify [--config <path>] [--policy]"));
        assert!(rendered.contains("platform assets publish [--config <path>] [--dry-run] [--yes]"));
        assert!(rendered.contains("platform import run <manifest-path> [--dry-run]"));
        assert!(rendered.contains("platform import cutover <manifest-path>"));
    }

    #[test]
    fn run_from_args_reports_live_auth_check_backend_initialization_failures() {
        let config_path = PathBuf::from("/tmp/davenda-cli-auth-check.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "check".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        assert!(
            error
                .to_string()
                .contains("failed to initialize the live auth check backend"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_reports_live_auth_check_config_load_failures_as_backend_initialization_failures()
     {
        let config_path = PathBuf::from("/tmp/davenda-cli-missing-auth-check.toml");

        let error = run_from_args([
            "auth".to_string(),
            "check".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        let rendered = error.to_string();
        assert!(
            rendered.contains("failed to initialize the live auth check backend"),
            "{rendered}"
        );
        assert!(
            rendered.contains("failed to load platform config"),
            "{rendered}"
        );
    }

    #[test]
    fn run_from_args_reports_live_auth_list_backend_initialization_failures() {
        let config_path = PathBuf::from("/tmp/davenda-cli-auth-list.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "list".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--relation".to_string(),
            "view".to_string(),
            "--namespace".to_string(),
            "page".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        assert!(
            error
                .to_string()
                .contains("failed to initialize the live auth list backend"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_reports_live_auth_lookup_backend_initialization_failures() {
        let config_path = PathBuf::from("/tmp/davenda-cli-auth-lookup.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "lookup".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
            "--relation".to_string(),
            "view".to_string(),
            "--subject-namespace".to_string(),
            "user".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        assert!(
            error
                .to_string()
                .contains("failed to initialize the live auth lookup backend"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_reports_live_auth_test_model_backend_initialization_failures() {
        let config_path = PathBuf::from("/tmp/davenda-cli-auth-test-model.toml");
        let spec_path = PathBuf::from("/tmp/davenda-cli-auth-test-model-spec.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();
        fs::write(
            &spec_path,
            r#"
[[case]]
name = "page read"
subject = "user:alice"
capability = "cms.page.read"
resource = "page:homepage"
expect = true
"#,
        )
        .unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "test-model".to_string(),
            spec_path.display().to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        assert!(
            error
                .to_string()
                .contains("failed to initialize the live auth test-model backend"),
            "{}",
            error
        );
    }

    #[test]
    fn load_auth_model_test_document_rejects_empty_specs() {
        let spec_path = PathBuf::from("/tmp/davenda-cli-auth-empty-spec.toml");
        fs::write(&spec_path, "").unwrap();

        let error = load_auth_model_test_document(&spec_path).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not define any `[[case]]` entries")
        );
    }

    #[test]
    fn run_from_args_renders_auth_bindings_for_the_configured_package() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "auth".to_string(),
            "bindings".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("auth bindings inspect"));
        assert!(rendered.contains("platform-default-auth"));
        assert!(rendered.contains("cms.page.read"));
        assert!(rendered.contains("page"));
    }

    #[test]
    fn run_from_args_renders_auth_package_inspect_for_the_configured_package() {
        let config_path = customer_app_fixture();
        let configured = fs::read_to_string(&config_path).unwrap().replace(
            "package = \"platform-default-auth\"",
            "package = \"shoppr-auth\"",
        );
        fs::write(&config_path, configured).unwrap();
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");
        let app_manifest = fs::read_to_string(&app_manifest_path).unwrap().replace(
            "package = \"platform-default-auth\"",
            "package = \"shoppr-auth\"",
        );
        fs::write(&app_manifest_path, app_manifest).unwrap();

        let rendered = run_from_args([
            "auth".to_string(),
            "package".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("auth package inspect"));
        assert!(rendered.contains("shoppr-auth"));
        assert!(rendered.contains("runtime_source"));
        assert!(rendered.contains("loaded auth package implementation"));
    }

    #[test]
    fn run_from_args_reports_disabled_auth_explain_from_live_config() {
        let config_path = PathBuf::from("/tmp/davenda-cli-disabled.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "explain".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn run_from_args_reports_live_auth_explain_config_load_failures_as_backend_initialization_failures()
     {
        let config_path = PathBuf::from("/tmp/davenda-cli-missing-auth-explain.toml");

        let error = run_from_args([
            "auth".to_string(),
            "explain".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        let rendered = error.to_string();
        assert!(
            rendered.contains("failed to initialize the live auth explain backend"),
            "{rendered}"
        );
        assert!(
            rendered.contains("failed to load platform config"),
            "{rendered}"
        );
    }

    #[test]
    fn run_from_args_uses_the_live_backend_when_deployment_enables_auth_explain() {
        let config_path = customer_app_fixture();
        let enabled_config = fs::read_to_string(&config_path)
            .unwrap()
            .replace("explain_api = false", "explain_api = true")
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"shoppr-auth\"",
            );
        fs::write(&config_path, enabled_config).unwrap();
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");
        let app_manifest = fs::read_to_string(&app_manifest_path).unwrap().replace(
            "package = \"platform-default-auth\"",
            "package = \"shoppr-auth\"",
        );
        fs::write(&app_manifest_path, app_manifest).unwrap();

        let error = run_from_args([
            "auth".to_string(),
            "explain".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 1);
        let message = error.to_string();
        assert!(
            message.contains("failed to initialize the live auth explain backend")
                || message.contains("failed to build the auth explanation"),
            "{message}"
        );
        assert!(!message.contains("auth explain API is disabled"));
    }

    #[test]
    fn run_from_args_plans_import_runs_from_a_manifest() {
        let fixture = import_fixture();

        let rendered = run_from_args([
            "import".to_string(),
            "run".to_string(),
            fixture.manifest_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Planned import run `wordpress-events`"));
        assert!(rendered.contains("users"));
        assert!(rendered.contains("events"));
        assert!(rendered.contains("fixtures/pages.json"));
    }

    #[test]
    fn run_from_args_executes_and_resumes_import_runs_from_a_manifest() {
        let fixture = import_fixture();

        let first = run_from_args([
            "import".to_string(),
            "run".to_string(),
            fixture.manifest_path.display().to_string(),
        ])
        .unwrap();
        assert!(first.contains("Executed import run `wordpress-events`"));
        assert!(first.contains("executed"));
        assert!(first.contains("staged"));
        assert!(first.contains("import.media.materialized"));
        assert!(fixture.journal_path.is_file());
        let staged_media_path = fixture
            .journal_path
            .with_extension("")
            .join("staged")
            .join("media.json");
        let staged_media = fs::read_to_string(&staged_media_path).unwrap();
        assert!(staged_media.contains("\"materialized\""));
        assert!(staged_media.contains("\"public_cdn\""));

        let second = run_from_args([
            "import".to_string(),
            "run".to_string(),
            fixture.manifest_path.display().to_string(),
        ])
        .unwrap();
        assert!(second.contains("Resumed import run `wordpress-events`"));
        assert!(second.contains("skipped_completed"));
    }

    #[test]
    fn run_from_args_rejects_import_targets_with_missing_modules() {
        let fixture = import_fixture();
        let invalid_manifest = fixture.root.join("imports").join("invalid-modules.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &invalid_manifest,
            manifest.replace(
                "expected_modules = [\"cms\", \"media\", \"events\"]",
                "expected_modules = [\"cms\", \"media\", \"events\", \"memberships\"]",
            ),
        )
        .unwrap();

        let error = run_from_args([
            "import".to_string(),
            "run".to_string(),
            invalid_manifest.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("import target expects modules not installed"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_rejects_import_targets_with_unsupported_locale() {
        let fixture = import_fixture();
        let invalid_manifest = fixture.root.join("imports").join("invalid-locale.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &invalid_manifest,
            manifest.replace(
                "customer_app_id = \"showcase-events\"",
                "customer_app_id = \"showcase-events\"\nlocale = \"fr\"",
            ),
        )
        .unwrap();

        let error = run_from_args([
            "import".to_string(),
            "run".to_string(),
            invalid_manifest.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("import manifest locale `fr` is not supported"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_rejects_import_manifests_with_missing_referenced_artifacts() {
        let fixture = import_fixture();
        let invalid_manifest = fixture.root.join("imports").join("invalid-artifacts.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &invalid_manifest,
            format!(
                "{manifest}\n[migration_artifacts]\ncapability_map = \"missing/capability-map.md\"\nauth_mapping = \"missing/auth-mapping.md\"\nredirect_plan = \"missing/redirect-plan.csv\"\nextraction_spec = \"missing/extraction-spec.md\"\ncutover_runbook = \"missing/cutover-runbook.md\"\n"
            ),
        )
        .unwrap();

        let error = run_from_args([
            "import".to_string(),
            "run".to_string(),
            invalid_manifest.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("failed to validate import manifest"),
            "{}",
            error
        );
        assert!(
            error
                .to_string()
                .contains("migration_artifacts.capability_map"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_reports_cutover_readiness_from_a_manifest() {
        let fixture = import_fixture();
        let cutover_manifest = fixture.root.join("imports").join("cutover.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &cutover_manifest,
            format!(
                "{manifest}\n[verification]\nrequired = [\"record_counts\"]\n[cutover]\nfreeze_legacy_writes = true\nswitch_method = \"dns\"\nhostnames = [\"shop.example.com\"]\nrequires_assets_publish = false\nrequires_migrate_apply = false\nrequires_storage_validation = true\nrequires_cache_warm = false\nobservation_window_minutes = 60\n\n[[cutover.rollback_triggers]]\nid = \"auth-failure\"\ndescription = \"Auth failure\"\n"
            ),
        )
        .unwrap();

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("import cutover"));
        assert!(rendered.contains("Cutover readiness for import run `wordpress-events`"));
        assert!(rendered.contains("release.doctor"));
        assert!(rendered.contains("auth.package.validate"));
        assert!(rendered.contains("storage.verify"));
        assert!(rendered.contains("legacy writes must be frozen"));
    }

    #[test]
    fn run_from_args_reports_migration_gated_cutover_auth_and_manual_customer_work() {
        let fixture = import_fixture();
        let cutover_manifest = fixture.root.join("imports").join("cutover-migrate.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &cutover_manifest,
            format!(
                "{manifest}\n[verification]\nrequired = [\"record_counts\"]\n[cutover]\nfreeze_legacy_writes = true\nswitch_method = \"dns\"\nhostnames = [\"shop.example.com\"]\nrequires_assets_publish = false\nrequires_migrate_apply = true\nrequires_storage_validation = false\nrequires_cache_warm = false\nobservation_window_minutes = 60\n\n[[cutover.rollback_triggers]]\nid = \"auth-failure\"\ndescription = \"Auth failure\"\n"
            ),
        )
        .unwrap();

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("migrate.apply"));
        assert!(rendered.contains("auth.package.validate"));
    }

    #[test]
    fn verification_readiness_executes_local_cutover_checks_when_declared() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let fixture = import_fixture();
        let cutover_manifest = write_cutover_observe_manifest_with_users_and_checks(
            &fixture,
            "cutover-local-verification.toml",
            60,
            &["record_counts", "auth_failures"],
            &["alice"],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://davenda:test@127.0.0.1:5432/davenda",
            );
            std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
        }
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _runtime_guard = tokio_runtime.enter();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();
        let support = build_cutover_verification_support(&runtime.built);
        let verification = manifest.verification.as_ref().unwrap();

        let (ready, detail) =
            evaluate_verification_readiness(verification, &runtime.built, &support);

        assert!(ready, "{detail}");
        assert!(detail.contains("local verification probes passed"));
        assert!(detail.contains("auth_failures"));
        unsafe {
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("REDIS_URL");
        }
    }

    #[test]
    fn verification_readiness_executes_local_transactional_cutover_checks_when_declared() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let fixture = import_fixture();
        let cutover_manifest = write_cutover_observe_manifest_with_routes_and_checks(
            &fixture,
            "cutover-local-transactional-verification.toml",
            60,
            "dns",
            &["record_counts", "transactional_journey_errors"],
            &["/", "/events/festival"],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://davenda:test@127.0.0.1:5432/davenda",
            );
            std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
        }
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _runtime_guard = tokio_runtime.enter();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();
        let support = build_cutover_verification_support(&runtime.built);
        let verification = manifest.verification.as_ref().unwrap();

        let (ready, detail) =
            evaluate_verification_readiness(verification, &runtime.built, &support);

        assert!(ready, "{detail}");
        assert!(detail.contains("transactional_journey_errors"));
        assert!(detail.contains("POST /events/festival/book"));
        unsafe {
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("REDIS_URL");
        }
    }

    #[test]
    fn verification_readiness_rejects_webhook_failures_without_matching_runtime_handler() {
        let fixture = import_fixture();
        let cutover_manifest = write_cutover_observe_manifest_with_webhooks_and_checks(
            &fixture,
            "cutover-local-webhook-verification.toml",
            60,
            &["record_counts", "webhook_failures"],
            &[("commerce.payment-provider", "payment.authorized", 0, 0)],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();
        let support = build_cutover_verification_support(&runtime.built);
        let verification = manifest.verification.as_ref().unwrap();

        let (ready, detail) =
            evaluate_verification_readiness(verification, &runtime.built, &support);

        assert!(!ready);
        assert!(detail.contains("webhook_failures"));
        assert!(detail.contains("commerce.payment-provider/payment.authorized"));
    }

    #[test]
    fn local_webhook_verification_supports_matching_runtime_handlers() {
        let verification = davenda_import::ImportVerification::default()
            .with_required("webhook_failures")
            .unwrap()
            .with_webhook(
                davenda_import::ImportWebhookVerification::new(
                    "commerce.payment-provider",
                    "payment.authorized",
                )
                .unwrap()
                .with_max_replay_rejections(1),
            );
        let support = CutoverVerificationSupport {
            fragment_probe: None,
            auth_probe: None,
            transactional_probes: Vec::new(),
            webhook_probes: vec![VerificationWebhookProbe {
                extension_id: "commerce.webhooks".to_string(),
                handler_id: "payment-authorized".to_string(),
                source: "commerce.payment-provider".to_string(),
                event: "payment.authorized".to_string(),
            }],
        };

        let checks = build_cutover_verification_checks(&verification, &support).unwrap();
        let probes = verify_local_webhook_failure_probes(&verification, &support).unwrap();
        let rendered = render_supported_verification_checks(&verification, checks);

        assert!(checks.webhook_failures);
        assert_eq!(
            probes,
            vec![
                "commerce.payment-provider/payment.authorized via commerce.webhooks:payment-authorized"
            ]
        );
        assert!(rendered.contains("webhook_failures(local+observe)"));
    }

    #[test]
    fn local_cache_leak_verification_is_reported_as_supported() {
        let verification = davenda_import::ImportVerification::default()
            .with_required("cache_leaks")
            .unwrap()
            .with_sample_route("/events")
            .unwrap()
            .with_sample_user("alice")
            .unwrap();
        let support = CutoverVerificationSupport::default();

        let checks = build_cutover_verification_checks(&verification, &support).unwrap();
        let rendered = render_supported_verification_checks(&verification, checks);

        assert!(checks.cache_leaks);
        assert!(rendered.contains("cache_leaks(local+observe)"));
    }

    #[test]
    fn verification_readiness_executes_local_cache_leak_checks_when_declared() {
        let fixture = import_fixture();
        let cutover_manifest = write_cutover_observe_manifest_with_users_and_checks(
            &fixture,
            "cutover-local-cache-leaks.toml",
            60,
            &["record_counts", "cache_leaks"],
            &["alice"],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();
        let support = build_cutover_verification_support(&runtime.built);
        let verification = manifest.verification.as_ref().unwrap();

        let (ready, detail) =
            evaluate_verification_readiness(verification, &runtime.built, &support);

        assert!(ready, "{detail}");
        assert!(detail.contains("cache_leaks(routes:"));
    }

    #[test]
    fn webhook_observation_ignores_events_before_the_observation_window() {
        let verification = davenda_import::ImportVerification::default().with_webhook(
            davenda_import::ImportWebhookVerification::new(
                "commerce.payment-provider",
                "payment.authorized",
            )
            .unwrap(),
        );
        let snapshot = WebhookObservationSnapshot {
            backend: davenda_runtime::WebhookObservationBackendKind::LocalSqlite,
            location: "local-sqlite:test".to_string(),
            path: None,
            entry_count: 2,
            status_counts: davenda_runtime::WebhookObservationStatusCounts {
                verification_failed: 2,
                ..Default::default()
            },
            recent_events: vec![
                WebhookObservationEvent {
                    id: 1,
                    recorded_at_unix_seconds: 99,
                    app_id: "showcase-events".to_string(),
                    source: "commerce.payment-provider".to_string(),
                    event: "payment.authorized".to_string(),
                    status: WebhookObservationStatus::VerificationFailed,
                    trace_id: "trace.old".to_string(),
                    principal_kind: "service_account".to_string(),
                    principal_id: Some("commerce.webhooks".to_string()),
                    detail: Some("old failure".to_string()),
                },
                WebhookObservationEvent {
                    id: 2,
                    recorded_at_unix_seconds: 100,
                    app_id: "showcase-events".to_string(),
                    source: "commerce.payment-provider".to_string(),
                    event: "payment.authorized".to_string(),
                    status: WebhookObservationStatus::VerificationFailed,
                    trace_id: "trace.window".to_string(),
                    principal_kind: "service_account".to_string(),
                    principal_id: Some("commerce.webhooks".to_string()),
                    detail: Some("current failure".to_string()),
                },
            ],
        };
        let mut routes = Vec::new();
        let mut failures = Vec::new();

        apply_webhook_observation_snapshot(
            &verification,
            &snapshot,
            100,
            &mut routes,
            &mut failures,
        );

        assert_eq!(routes.len(), 1);
        assert!(routes[0].outcome.contains("verification_failures=1"));
        assert!(failures.contains(
            &"webhook `commerce.payment-provider/payment.authorized` observed 1 verification failure(s) after observation started (max 0)"
                .to_string()
        ));
    }

    #[test]
    fn webhook_observation_tracks_replay_rejections_within_budget() {
        let verification = davenda_import::ImportVerification::default().with_webhook(
            davenda_import::ImportWebhookVerification::new(
                "commerce.payment-provider",
                "payment.authorized",
            )
            .unwrap()
            .with_max_replay_rejections(1),
        );
        let mut routes = Vec::new();
        let mut failures = Vec::new();

        apply_webhook_observation_events(
            &verification,
            &[WebhookObservationEvent {
                id: 1,
                recorded_at_unix_seconds: 100,
                app_id: "showcase-events".to_string(),
                source: "commerce.payment-provider".to_string(),
                event: "payment.authorized".to_string(),
                status: WebhookObservationStatus::ReplayRejected,
                trace_id: "trace.replay".to_string(),
                principal_kind: "service_account".to_string(),
                principal_id: Some("commerce.webhooks".to_string()),
                detail: Some("duplicate delivery".to_string()),
            }],
            &mut routes,
            &mut failures,
        );

        assert_eq!(routes.len(), 1);
        assert!(routes[0].outcome.contains("replay_rejections=1"));
        assert!(routes[0].outcome.contains("within_budget"));
        assert!(failures.is_empty());
    }

    #[test]
    fn run_from_args_executes_cutover_preparation_and_persists_a_journal() {
        let fixture = import_fixture();
        let cutover_manifest = fixture.root.join("imports").join("cutover-apply.toml");
        let config_path = fixture.root.join("config").join("platform.toml");
        let app_manifest_path = fixture
            .root
            .join("apps")
            .join("showcase-events")
            .join("app.toml");
        let config = fs::read_to_string(&config_path).unwrap().replace(
            "enabled = [\"cms\", \"media\", \"events\"]",
            "enabled = [\"cms\", \"media\", \"events\", \"admin\", \"ops\"]",
        );
        fs::write(&config_path, config).unwrap();
        let app_manifest = fs::read_to_string(&app_manifest_path).unwrap().replace(
            "enabled = [\"cms\", \"media\", \"events\"]",
            "enabled = [\"cms\", \"media\", \"events\", \"admin\", \"ops\"]",
        );
        fs::write(&app_manifest_path, app_manifest).unwrap();
        fs::write(
            &cutover_manifest,
            format!(
                "run_id = \"wordpress-events\"\nsource_system = \"wordpress\"\nsnapshot_at = \"2026-03-19T00:00:00Z\"\ncustomer_app_id = \"showcase-events\"\nmodules = [\"media\"]\npublication_mode = \"publish_validated\"\nasset_storage_default = \"public_upload\"\n\n[target]\napp_manifest = \"../apps/showcase-events/app.toml\"\nplatform_config = \"../config/platform.toml\"\nexpected_modules = [\"media\", \"admin\", \"ops\"]\n\n[verification]\nrequired = [\"record_counts\"]\n\n[cutover]\nfreeze_legacy_writes = true\nswitch_method = \"dns\"\nhostnames = [\"shop.example.com\"]\nrequires_assets_publish = false\nrequires_migrate_apply = false\nrequires_storage_validation = true\nrequires_cache_warm = false\nobservation_window_minutes = 60\n\n[[cutover.rollback_triggers]]\nid = \"auth-failure\"\ndescription = \"Auth failure\"\n\n[[importers]]\nid = \"media\"\nphase = 20\nresource_kind = \"asset\"\ndescription = \"Import media\"\nsource_path = \"fixtures/media.json\"\n"
            ),
        )
        .unwrap();

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
            "--legacy-freeze-confirmed".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Cutover preparation for import run `wordpress-events`"));
        assert!(rendered.contains("prepared"));
        assert!(rendered.contains("final.import"));
        assert!(rendered.contains("storage.verify"));
        assert!(rendered.contains("cutover.readiness"));

        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"prepared\""));
        assert!(journal.contains("\"final.import\""));
        assert!(journal.contains("\"storage.verify\""));

        let rerun = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
            "--legacy-freeze-confirmed".to_string(),
        ])
        .unwrap();
        assert!(rerun.contains("prepared"));
    }

    #[test]
    fn run_from_args_executes_dns_cutover_switch_and_persists_rollback_state() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest =
            write_cutover_observe_manifest(&fixture, "cutover-switch-dns.toml", 60);

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let switched = run_dns_cutover_switch(&cutover_manifest, "https://shop.example.com", &dns);
        assert!(switched.contains("Cutover switch"));
        assert_eq!(
            dns.server.record("shop.example.com").content,
            dns.dns_target
        );

        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"switch_execution\""));
        assert!(journal.contains("\"previous_content\": \"legacy-origin.example.net\""));
        assert!(journal.contains(&format!("\"current_content\": \"{}\"", dns.dns_target)));
    }

    #[test]
    fn run_from_args_dry_runs_load_balancer_cutover_switch_without_mutating_provider_state() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let context = configure_load_balancer_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_for_switch_method(
            &fixture,
            "cutover-switch-load-balancer-dry-run.toml",
            60,
            "load-balancer",
            &["record_counts"],
        );

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let rendered = run_traffic_target_cutover_switch_dry_run(
            &cutover_manifest,
            "https://shop.example.com",
            &context,
        );

        assert!(rendered.contains("Planned cutover switch"));
        assert!(rendered.contains("no provider state or cutover journal was modified"));
        assert!(rendered.contains("load_balancer"));
        assert_eq!(
            context
                .server
                .load_balancer(&context.resource_id)
                .default_pools,
            vec!["legacy-origin-pool".to_string()]
        );

        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"prepared\""));
        assert!(journal.contains("\"switch_confirmed_at_unix_seconds\": null"));
        assert!(!journal.contains("\"resource_kind\": \"load_balancer\""));
    }

    #[test]
    fn run_from_args_executes_load_balancer_cutover_switch_and_rollback() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let context = configure_load_balancer_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_for_switch_method(
            &fixture,
            "cutover-switch-load-balancer.toml",
            60,
            "load-balancer",
            &["record_counts"],
        );

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let switched = run_traffic_target_cutover_switch(
            &cutover_manifest,
            "https://shop.example.com",
            &context,
        );
        assert!(switched.contains("Cutover switch"));
        assert_eq!(
            context
                .server
                .load_balancer(&context.resource_id)
                .default_pools,
            vec![context.target.clone()]
        );

        let rolled_back = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--rollback".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--reason".to_string(),
            "load balancer rollback".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();
        assert!(rolled_back.contains("Cutover rollback"));
        assert_eq!(
            context
                .server
                .load_balancer(&context.resource_id)
                .default_pools,
            vec!["legacy-origin-pool".to_string()]
        );
    }

    #[test]
    fn run_from_args_confirms_load_balancer_switch_with_a_read_after_stale_put_response() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let context = configure_load_balancer_cutover_for_import_fixture(&fixture);
        context
            .server
            .set_load_balancer_update_result(CloudflareTestLoadBalancer::new(
                &context.resource_id,
                "legacy-origin-pool",
            ));
        let cutover_manifest = write_cutover_observe_manifest_for_switch_method(
            &fixture,
            "cutover-switch-load-balancer-stale-put.toml",
            60,
            "load-balancer",
            &["record_counts"],
        );

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let switched = run_traffic_target_cutover_switch(
            &cutover_manifest,
            "https://shop.example.com",
            &context,
        );
        assert!(switched.contains("Cutover switch"));
        assert_eq!(
            context
                .server
                .load_balancer(&context.resource_id)
                .default_pools,
            vec![context.target.clone()]
        );
    }

    #[test]
    fn run_from_args_executes_cdn_origin_cutover_switch() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let context = configure_cdn_origin_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_for_switch_method(
            &fixture,
            "cutover-switch-cdn-origin.toml",
            60,
            "cdn-origin",
            &["record_counts"],
        );

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let switched = run_traffic_target_cutover_switch(
            &cutover_manifest,
            "https://shop.example.com",
            &context,
        );
        assert!(switched.contains("Cutover switch"));
        assert_eq!(
            context.server.origin_rule(&context.resource_id).origin,
            context.target
        );
    }

    #[test]
    fn run_from_args_executes_routing_cutover_switch() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let context = configure_routing_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_for_switch_method(
            &fixture,
            "cutover-switch-routing.toml",
            60,
            "routing",
            &["record_counts"],
        );

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let switched = run_traffic_target_cutover_switch(
            &cutover_manifest,
            "https://shop.example.com",
            &context,
        );
        assert!(switched.contains("Cutover switch"));
        assert_eq!(
            context.server.routing_rule(&context.resource_id).service,
            context.target
        );
    }

    #[test]
    fn run_from_args_observes_a_prepared_cutover_until_it_passes() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cache_route_candidates = ["/", "/events", "/en-GB/pages/home"];
        let cutover_manifest = write_cutover_observe_manifest_with_sample_routes(
            &fixture,
            "cutover-observe-pass.toml",
            0,
            &["record_counts"],
            &cache_route_candidates,
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();
        let Some(cache_route) = choose_cache_probe_route(&runtime.built, &cache_route_candidates)
        else {
            return;
        };

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            cache_route_candidates
                .iter()
                .map(|route| {
                    let response = if *route == cache_route {
                        cache_probe_response(
                            &runtime.built,
                            route,
                            format!("<html><body>{route}</body></html>"),
                        )
                    } else {
                        LiveProbeResponse::html(200, format!("<html><body>{route}</body></html>"))
                    };
                    ((*route).to_string(), response)
                })
                .collect(),
        );
        let switched = run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);
        assert!(switched.contains("Cutover switch"));

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("passed against"));
        assert!(rendered.contains("switch.confirmed"));
        assert!(rendered.contains("cutover.observe"));
        assert!(rendered.contains("cache_ok"));

        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"observation_passed\""));
        assert!(journal.contains("\"switch_confirmed_at_unix_seconds\""));
        assert!(journal.contains(probe_server.base_url()));
    }

    #[test]
    fn run_from_args_marks_cutover_observation_failures_for_rollback_review() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest =
            write_cutover_observe_manifest(&fixture, "cutover-observe-fail.toml", 0);
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([
                (
                    "/".to_string(),
                    cache_probe_response(&runtime.built, "/", "<html><body>/</body></html>"),
                ),
                (
                    "/events".to_string(),
                    LiveProbeResponse::html(500, "<html><body>/events</body></html>"),
                ),
            ]),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);
        let error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(error.to_string().contains("requires rollback review"));
        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"rollback_required\""));
        assert!(journal.contains("unexpected status 500"));
    }

    #[test]
    fn run_from_args_observation_executes_canonical_and_media_checks() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_with_checks(
            &fixture,
            "cutover-observe-verification.toml",
            0,
            &[
                "record_counts",
                "route_resolution",
                "canonical_urls",
                "media_reachability",
            ],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([
                (
                    "/".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/",
                        r#"<html><head><link rel="canonical" href="/" /></head><body><img src="/media/home.jpg" /></body></html>"#,
                    ),
                ),
                (
                    "/events".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/events",
                        r#"<html><head><link rel="canonical" href="/events" /></head><body><img src="/media/festival.jpg" /></body></html>"#,
                    ),
                ),
                (
                    "/media/home.jpg".to_string(),
                    LiveProbeResponse::binary(200, b"home".to_vec()),
                ),
                (
                    "/media/festival.jpg".to_string(),
                    LiveProbeResponse::binary(200, b"festival".to_vec()),
                ),
            ]),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("canonical_ok"));
        assert!(rendered.contains("media_ok(1)"));
    }

    #[test]
    fn run_from_args_marks_missing_canonical_or_media_as_rollback_required() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_with_checks(
            &fixture,
            "cutover-observe-verification-fail.toml",
            0,
            &[
                "record_counts",
                "route_resolution",
                "canonical_urls",
                "media_reachability",
            ],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([
                (
                    "/".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/",
                        r#"<html><head><link rel="canonical" href="/" /></head><body><img src="/media/home.jpg" /></body></html>"#,
                    ),
                ),
                (
                    "/events".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/events",
                        r#"<html><head></head><body><img src="/media/festival.jpg" /></body></html>"#,
                    ),
                ),
                (
                    "/media/home.jpg".to_string(),
                    LiveProbeResponse::binary(200, b"home".to_vec()),
                ),
                (
                    "/media/festival.jpg".to_string(),
                    LiveProbeResponse::binary(404, Vec::new()),
                ),
            ]),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);

        let _error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"rollback_required\""));
    }

    #[test]
    fn run_from_args_observation_executes_auth_failure_checks() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_with_users_and_checks(
            &fixture,
            "cutover-observe-auth-failures.toml",
            0,
            &["record_counts", "auth_failures"],
            &["alice"],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([
                (
                    "/".to_string(),
                    cache_probe_response(&runtime.built, "/", "<html><body>home</body></html>"),
                ),
                (
                    "/events".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/events",
                        "<html><body>events</body></html>",
                    ),
                ),
                (
                    "/admin/pages/preview".to_string(),
                    LiveProbeResponse::html(401, "<html><body>unauthorized</body></html>"),
                ),
            ]),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("auth_gate_ok"));
    }

    #[test]
    fn run_from_args_marks_cache_leak_observation_failures_for_rollback_review() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_with_users_and_checks(
            &fixture,
            "cutover-observe-cache-leaks.toml",
            0,
            &["record_counts", "cache_leaks"],
            &["alice"],
        );

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            ["/", "/events"]
                .iter()
                .map(|route| {
                    let response = if *route == "/" {
                        LiveProbeResponse::html(200, "<html><body>public storefront</body></html>")
                            .with_header("Cache-Control", "public, max-age=300")
                            .with_header("Surrogate-Key", "route:home locale:en")
                            .with_authenticated_response(
                                LiveProbeResponse::html(
                                    200,
                                    "<html><body>hello alice</body></html>",
                                )
                                .with_header("Cache-Control", "public, max-age=300")
                                .with_header("Surrogate-Key", "route:home locale:en"),
                            )
                    } else {
                        LiveProbeResponse::html(200, format!("<html><body>{route}</body></html>"))
                    };
                    ((*route).to_string(), response)
                })
                .collect(),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://davenda:test@127.0.0.1:5432/davenda",
            );
            std::env::set_var("REDIS_URL", "redis://127.0.0.1:6379");
            std::env::set_var("DAVENDA_COOKIE_SECRET", "01234567012345670123456701234567");
            std::env::set_var("DAVENDA_CSRF_SECRET", "76543210765432107654321076543210");
            std::env::set_var(CUTOVER_SYNTHETIC_SESSION_ENV, "1");
        }
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _runtime_guard = tokio_runtime.enter();

        let error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(
            error.to_string().contains("requires rollback review"),
            "{error}"
        );
        assert!(
            error
                .to_string()
                .contains("retaining the anonymous cache policy"),
            "{error}"
        );
        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"rollback_required\""));
        assert!(journal.contains("retaining the anonymous cache policy"));
        unsafe {
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("REDIS_URL");
            std::env::remove_var("DAVENDA_COOKIE_SECRET");
            std::env::remove_var("DAVENDA_CSRF_SECRET");
            std::env::remove_var(CUTOVER_SYNTHETIC_SESSION_ENV);
        }
    }

    #[test]
    fn run_from_args_observation_executes_transactional_journey_checks() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_with_routes_and_checks(
            &fixture,
            "cutover-observe-transactional.toml",
            0,
            "dns",
            &["record_counts", "transactional_journey_errors"],
            &["/", "/events/festival"],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([
                (
                    "/".to_string(),
                    cache_probe_response(&runtime.built, "/", "<html><body>home</body></html>"),
                ),
                (
                    "/events/festival".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/events/festival",
                        "<html><body>festival</body></html>",
                    ),
                ),
                (
                    "/events/festival/book".to_string(),
                    LiveProbeResponse::html(202, "<html><body>queued</body></html>"),
                ),
            ]),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);

        let rendered = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("POST /events/festival/book"));
        assert!(rendered.contains("transactional_ok(202)"));
    }

    #[test]
    fn run_from_args_marks_transactional_journey_failures_for_rollback_review() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest = write_cutover_observe_manifest_with_routes_and_checks(
            &fixture,
            "cutover-observe-transactional-fail.toml",
            0,
            "dns",
            &["record_counts", "transactional_journey_errors"],
            &["/", "/events/festival"],
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();
        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([
                (
                    "/".to_string(),
                    cache_probe_response(&runtime.built, "/", "<html><body>home</body></html>"),
                ),
                (
                    "/events/festival".to_string(),
                    cache_probe_response(
                        &runtime.built,
                        "/events/festival",
                        "<html><body>festival</body></html>",
                    ),
                ),
                (
                    "/events/festival/book".to_string(),
                    LiveProbeResponse::html(500, "<html><body>error</body></html>"),
                ),
            ]),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);

        let error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(error.to_string().contains("requires rollback review"));
        assert!(
            error
                .to_string()
                .contains("transactional verification route `/events/festival/book` returned 500")
        );
    }

    #[test]
    fn run_from_args_marks_cache_header_mismatches_for_rollback_review() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cache_route_candidates = ["/", "/events", "/en-GB/pages/home"];
        let cutover_manifest = write_cutover_observe_manifest_with_sample_routes(
            &fixture,
            "cutover-observe-cache-fail.toml",
            0,
            &["record_counts"],
            &cache_route_candidates,
        );
        let manifest = ImportManifest::from_file(&cutover_manifest).unwrap();
        let manifest_root = cutover_manifest.parent().unwrap();

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let runtime = build_import_runtime_context(manifest_root, &manifest)
            .unwrap()
            .unwrap();
        let Some(cache_route) = choose_cache_probe_route(&runtime.built, &cache_route_candidates)
        else {
            return;
        };

        let mismatched_events = cache_probe_response(
            &runtime.built,
            &cache_route,
            format!("<html><body>{cache_route}</body></html>"),
        )
        .with_header("Cache-Control", "public,max-age=1,stale-while-revalidate=1");
        let probe_server = LiveProbeTestServer::spawn_with_responses(
            "healthy",
            "healthy",
            false,
            cache_route_candidates
                .iter()
                .map(|route| {
                    let response = if *route == cache_route {
                        mismatched_events.clone()
                    } else {
                        LiveProbeResponse::html(200, format!("<html><body>{route}</body></html>"))
                    };
                    ((*route).to_string(), response)
                })
                .collect(),
        );
        run_dns_cutover_switch(&cutover_manifest, probe_server.base_url(), &dns);

        let error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(error.to_string().contains("cache headers"));
        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"rollback_required\""));
        assert!(journal.contains(&format!("cache headers for route `{cache_route}`")));
    }

    #[test]
    fn run_from_args_records_cutover_rollbacks_after_the_live_switch() {
        let fixture = import_fixture();
        enable_admin_and_ops_for_import_fixture(&fixture);
        let dns = configure_dns_cutover_for_import_fixture(&fixture);
        let cutover_manifest =
            write_cutover_observe_manifest(&fixture, "cutover-rollback.toml", 60);

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();
        let switched = run_dns_cutover_switch(&cutover_manifest, "https://shop.example.com", &dns);
        assert!(switched.contains("Cutover switch"));

        let rolled_back = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--rollback".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--reason".to_string(),
            "systemic auth failure".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rolled_back.contains("rollback"));
        assert!(rolled_back.contains("systemic auth failure"));

        let journal_path = cutover_journal_path(
            &cutover_manifest,
            &davenda_import::ImportRunId::new("wordpress-events").unwrap(),
        );
        let journal = fs::read_to_string(journal_path).unwrap();
        assert!(journal.contains("\"state\": \"rolled_back\""));
        assert!(journal.contains("\"rollback_confirmed_at_unix_seconds\""));
        assert!(journal.contains("systemic auth failure"));
        assert_eq!(
            dns.server.record("shop.example.com").content,
            "legacy-origin.example.net"
        );
    }

    #[test]
    fn run_from_args_requires_legacy_freeze_confirmation_for_cutover_apply() {
        let fixture = import_fixture();
        let cutover_manifest = fixture.root.join("imports").join("cutover-apply.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &cutover_manifest,
            format!(
                "publication_mode = \"publish_validated\"\nsite = \"showcase-events\"\n{manifest}\n[verification]\nrequired = [\"record_counts\"]\n[cutover]\nfreeze_legacy_writes = true\nswitch_method = \"dns\"\nhostnames = [\"shop.example.com\"]\nrequires_assets_publish = false\nrequires_migrate_apply = false\nrequires_storage_validation = true\nrequires_cache_warm = false\nobservation_window_minutes = 60\n\n[[cutover.rollback_triggers]]\nid = \"auth-failure\"\ndescription = \"Auth failure\"\n"
            ),
        )
        .unwrap();

        let error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`import cutover --apply` requires `--legacy-freeze-confirmed`"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_rejects_cutover_apply_when_import_manifest_is_not_publish_validated() {
        let fixture = import_fixture();
        let cutover_manifest = fixture.root.join("imports").join("cutover-stage-only.toml");
        let manifest = fs::read_to_string(&fixture.manifest_path).unwrap();
        fs::write(
            &cutover_manifest,
            format!(
                "{manifest}\n[verification]\nrequired = [\"record_counts\"]\n[cutover]\nfreeze_legacy_writes = false\nswitch_method = \"dns\"\nhostnames = [\"shop.example.com\"]\nrequires_assets_publish = false\nrequires_migrate_apply = false\nrequires_storage_validation = true\nrequires_cache_warm = false\nobservation_window_minutes = 60\n\n[[cutover.rollback_triggers]]\nid = \"auth-failure\"\ndescription = \"Auth failure\"\n"
            ),
        )
        .unwrap();

        let error = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("cutover `wordpress-events` is not executable yet"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_validates_config_and_renders_a_report() {
        let config_path = PathBuf::from("/tmp/davenda-cli-config-validate.toml");
        fs::write(&config_path, DISABLED_EXPLAIN_CONFIG).unwrap();

        let rendered = run_from_args([
            "config".to_string(),
            "validate".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("config validate"));
        assert!(rendered.contains("Validated effective platform configuration"));
        assert!(rendered.contains("showcase-events"));
    }

    #[test]
    fn run_from_args_renders_module_list_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "module".to_string(),
            "list".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module list"));
        assert!(rendered.contains("cms"));
    }

    #[test]
    fn run_from_args_renders_module_inspect_for_an_installed_module() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "module".to_string(),
            "inspect".to_string(),
            "cms".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module inspect"));
        assert!(rendered.contains("Inspected module `cms`"));
        assert!(rendered.contains("capability_contracts"));
        assert!(rendered.contains("module.version.unpinned"));
    }

    #[test]
    fn run_from_args_renders_module_inspect_for_an_available_official_module() {
        let config_path = customer_app_fixture_with_modules(&["cms"]);

        let rendered = run_from_args([
            "module".to_string(),
            "inspect".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module inspect"));
        assert!(rendered.contains("Inspected module `media`"));
        assert!(rendered.contains("status: warning"));
        assert!(rendered.contains("status: available"));
        assert!(rendered.contains("module=media installed=false"));
        assert!(rendered.contains("module.installation.available"));
        assert!(!rendered.contains("module.version.unpinned"));
    }

    #[test]
    fn run_from_args_warns_when_available_module_has_missing_required_dependencies() {
        let config_path = customer_app_fixture_with_modules(&["cms"]);

        let rendered = run_from_args([
            "module".to_string(),
            "inspect".to_string(),
            "memberships".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Inspected module `memberships`"));
        assert!(rendered.contains("module.dependencies.missing"));
        assert!(rendered.contains("required module dependencies are missing: commerce"));
        assert!(rendered.contains("status: warning"));
    }

    #[test]
    fn run_from_args_requires_confirmation_for_module_install() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "module".to_string(),
            "install".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`module install` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_renders_module_install_dry_run_and_leaves_files_unchanged() {
        let config_path = customer_app_fixture();
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");
        let original_config = fs::read_to_string(&config_path).unwrap();
        let original_manifest = fs::read_to_string(&app_manifest_path).unwrap();

        let rendered = run_from_args([
            "module".to_string(),
            "install".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module install"));
        assert!(rendered.contains("Planned module `media` to be installed"));
        assert!(rendered.contains("planned"));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original_config);
        assert_eq!(
            fs::read_to_string(&app_manifest_path).unwrap(),
            original_manifest
        );
    }

    #[test]
    fn run_from_args_installs_module_in_config_and_manifest() {
        let config_path = customer_app_fixture();
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");

        let rendered = run_from_args([
            "module".to_string(),
            "install".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Module `media` installed"));
        let config_contents = fs::read_to_string(&config_path).unwrap();
        let manifest_contents = fs::read_to_string(&app_manifest_path).unwrap();
        assert!(config_contents.contains("\"media\""));
        assert!(manifest_contents.contains("\"media\""));
    }

    #[test]
    fn run_from_args_requires_confirmation_for_module_enable() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "module".to_string(),
            "enable".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`module enable` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_renders_module_enable_dry_run_and_leaves_files_unchanged() {
        let config_path = customer_app_fixture();
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");
        let original_config = fs::read_to_string(&config_path).unwrap();
        let original_manifest = fs::read_to_string(&app_manifest_path).unwrap();

        let rendered = run_from_args([
            "module".to_string(),
            "enable".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module enable"));
        assert!(rendered.contains("Planned module `media` to be enabled"));
        assert!(rendered.contains("planned"));
        assert_eq!(fs::read_to_string(&config_path).unwrap(), original_config);
        assert_eq!(
            fs::read_to_string(&app_manifest_path).unwrap(),
            original_manifest
        );
    }

    #[test]
    fn run_from_args_enables_module_in_config_and_manifest() {
        let config_path = customer_app_fixture();
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");

        let rendered = run_from_args([
            "module".to_string(),
            "enable".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Module `media` enabled"));
        let config_contents = fs::read_to_string(&config_path).unwrap();
        let manifest_contents = fs::read_to_string(&app_manifest_path).unwrap();
        assert!(config_contents.contains("\"media\""));
        assert!(manifest_contents.contains("\"media\""));
    }

    #[test]
    fn run_from_args_disables_module_in_config_and_manifest() {
        let config_path = customer_app_fixture_with_modules(&["cms", "media"]);
        let app_manifest_path = config_path
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("apps")
            .join("showcase-events")
            .join("app.toml");

        let rendered = run_from_args([
            "module".to_string(),
            "disable".to_string(),
            "media".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Module `media` disabled"));
        let config_contents = fs::read_to_string(&config_path).unwrap();
        let manifest_contents = fs::read_to_string(&app_manifest_path).unwrap();
        assert!(!config_contents.contains("\"media\""));
        assert!(!manifest_contents.contains("\"media\""));
        assert!(config_contents.contains("\"cms\""));
        assert!(manifest_contents.contains("\"cms\""));
    }

    #[test]
    fn run_from_args_rejects_module_disable_that_would_leave_no_modules_enabled() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "module".to_string(),
            "disable".to_string(),
            "cms".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--yes".to_string(),
        ])
        .unwrap_err();

        assert!(error.to_string().contains("updated platform config"));
        assert!(
            error
                .to_string()
                .contains("at least one module must be enabled")
        );
    }

    #[test]
    fn run_from_args_validates_the_configured_auth_package_against_installed_modules() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "auth".to_string(),
            "package".to_string(),
            "validate".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("auth package validate"));
        assert!(rendered.contains("platform-default-auth"));
        assert!(rendered.contains("cms"));
        assert!(rendered.contains("valid"));
    }

    #[test]
    fn run_from_args_renders_migration_plan_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "migrate".to_string(),
            "plan".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("migrate plan"));
        assert!(rendered.contains("customer_app:showcase-events"));
    }

    #[test]
    fn run_from_args_requires_confirmation_for_migrate_apply() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "migrate".to_string(),
            "apply".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`migrate apply` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_renders_migrate_apply_dry_run_for_executable_module_migrations() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "migrate".to_string(),
            "apply".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("migrate apply"));
        assert!(rendered.contains("module:cms"));
        assert!(rendered.contains("status: planned"));
        assert!(rendered.contains("auth:platform-default-auth"));
        assert!(rendered.contains("customer_app:showcase-events"));
        assert!(rendered.contains("manual-runbook"));
    }

    #[test]
    fn run_from_args_reports_migrate_apply_database_failures_without_panicking() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();
        let original_database_url = std::env::var("DATABASE_URL").ok();

        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://davenda:devpass@127.0.0.1:1/davenda",
            );
        }

        let outcome = std::panic::catch_unwind(|| {
            run_from_args([
                "migrate".to_string(),
                "apply".to_string(),
                "--config".to_string(),
                config_path.display().to_string(),
                "--yes".to_string(),
            ])
        });

        match original_database_url {
            Some(database_url) => unsafe {
                std::env::set_var("DATABASE_URL", database_url);
            },
            None => unsafe {
                std::env::remove_var("DATABASE_URL");
            },
        }

        let error = outcome
            .expect("migrate apply should return an error instead of panicking")
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("failed to read applied migrations")
        );
    }

    #[test]
    fn build_dev_server_runtime_plan_loads_customer_templates_from_app_root() {
        let config_path = customer_app_fixture();

        let plan = build_dev_server_runtime_plan(&config_path).unwrap();
        let rendered = format!("{:?}", plan.template.registry);

        assert!(rendered.contains("pages/home"));
        assert!(plan.http.routes.iter().any(|route| route.path == "/"));
        assert!(plan.handlers.contains_key("home"));
    }

    #[test]
    fn build_dev_server_runtime_plan_carries_theme_asset_manifest() {
        let fixture = customer_app_fixture_with_assets(true);

        let plan = build_dev_server_runtime_plan(&fixture.config_path).unwrap();
        let manifest = plan
            .theme_asset_manifest
            .as_ref()
            .expect("dev server plan should include published theme assets");

        assert!(
            manifest
                .entries()
                .any(|(logical_path, _)| logical_path == "theme/assets/site.css")
        );
    }

    #[test]
    fn build_dev_server_async_runtime_uses_the_multithread_tokio_flavor() {
        let runtime = build_dev_server_async_runtime().unwrap();

        assert_eq!(
            runtime.handle().runtime_flavor(),
            tokio::runtime::RuntimeFlavor::MultiThread
        );
    }

    #[test]
    fn run_from_args_renders_release_doctor_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "release".to_string(),
            "doctor".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("release doctor"));
        assert!(rendered.contains("showcase-events"));
    }

    #[test]
    fn run_from_args_renders_release_plan_from_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "release".to_string(),
            "plan".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("release plan"));
        assert!(rendered.contains("compatibility"));
        assert!(rendered.contains("migration"));
    }

    #[test]
    fn run_from_args_plans_cache_warm_for_sample_customer_app_routes() {
        let rendered = run_from_args([
            "cache".to_string(),
            "warm".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
            "--scope".to_string(),
            "public".to_string(),
            "--route".to_string(),
            "/en-GB/pages/home".to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("cache warm"));
        assert!(rendered.contains("status: planned"));
        assert!(rendered.contains("/en-GB/pages/home"));
    }

    #[test]
    fn run_from_args_renders_cache_inspect_for_sample_customer_app_routes() {
        let rendered = run_from_args([
            "cache".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
            "--route".to_string(),
            "/en-GB/pages/home".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("cache"));
        assert!(rendered.contains("inspect"));
        assert!(rendered.contains("/en-GB/pages/home"));
        assert!(
            rendered.contains("lookup: miss")
                || rendered.contains("lookup: fresh")
                || rendered.contains("lookup: backend_unavailable")
        );
    }

    #[test]
    fn run_from_args_plans_cache_invalidation_for_explicit_tags() {
        let rendered = run_from_args([
            "cache".to_string(),
            "invalidate".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
            "--tag".to_string(),
            "route:pages.home".to_string(),
            "--tag".to_string(),
            "locale:en-GB".to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("cache invalidation"));
        assert!(rendered.contains("status: planned"));
        assert!(rendered.contains("route:pages.home"));
    }

    #[test]
    fn run_from_args_renders_tls_status_for_a_customer_app_runtime_plan() {
        ensure_test_tls_material_key();
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "tls".to_string(),
            "status".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("tls status"));
        assert!(rendered.contains("showcase-events"));
        assert!(rendered.contains("externally terminated"));
    }

    #[test]
    fn run_from_args_renders_jobs_status_for_a_customer_app_runtime_plan() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "jobs".to_string(),
            "status".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("jobs status"));
        assert!(rendered.contains("showcase-events"));
        assert!(rendered.contains("jobs.work"));
        assert!(rendered.contains("registered_jobs"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn run_from_args_rejects_jobs_run_for_dead_letter_queue() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "jobs".to_string(),
            "run".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--queue".to_string(),
            "jobs.dead-letter".to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("`jobs run` cannot execute the dead-letter queue")
        );
    }

    #[test]
    fn run_from_args_warns_when_jobs_run_cannot_access_live_state() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "jobs".to_string(),
            "run".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("jobs run"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn run_from_args_rejects_jobs_ready_without_live_coordinator_state() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();
        let original_database_url = std::env::var("DATABASE_URL").ok();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }

        let error = run_from_args([
            "jobs".to_string(),
            "ready".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--queue".to_string(),
            "jobs.work".to_string(),
            "--limit".to_string(),
            "10".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("live jobs coordinator state is required to inspect ready jobs")
        );
        assert!(error.to_string().contains("DATABASE_URL"));

        if let Some(database_url) = original_database_url {
            unsafe {
                std::env::set_var("DATABASE_URL", database_url);
            }
        }
    }

    #[test]
    fn run_from_args_rejects_jobs_ready_for_unknown_queue_filters() {
        let config_path = customer_app_fixture();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();

        let error = run_from_args([
            "jobs".to_string(),
            "ready".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--queue".to_string(),
            "jobs.not-real".to_string(),
            "--limit".to_string(),
            "10".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("queue filter `jobs.not-real` is not defined")
        );
        assert!(error.to_string().contains("jobs.work"));
    }

    #[test]
    fn build_jobs_ready_report_keeps_normal_backlog_at_ok_status() {
        let config_path = customer_app_fixture_with_modules(&["cms", "media"]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = runtime.enter();
        let built = build_customer_app_runtime_context(&config_path, true).unwrap();
        let topology = built.runtime_plan.runtime.jobs.describe().clone();
        let now = JobInstant::from_unix_seconds(unix_timestamp_now().unwrap());
        let job_id = davenda_jobs::JobId::new("job:ready-probe").unwrap();
        let job_name = davenda_jobs::JobName::new("ops.search.reindex".to_string()).unwrap();
        let ready_jobs = vec![davenda_jobs::QueuedJobRecord {
            spec: davenda_jobs::JobSpec::new(
                job_id.clone(),
                job_name.clone(),
                topology.work_queue.clone(),
                "cli_ready_probe",
            )
            .unwrap()
            .with_idempotency_key(davenda_jobs::IdempotencyKey::new("cli_ready_probe").unwrap()),
            attempts: 1,
            enqueued_at: now,
        }];

        let report = build_jobs_ready_report(
            built.manifest.id.as_str(),
            &topology,
            built.runtime_plan.runtime.jobs.backend,
            built.runtime_plan.runtime.jobs.default_retry_limit,
            &ready_jobs,
            Some(topology.work_queue.as_str()),
            10,
        )
        .unwrap();

        assert_eq!(report.status, ReportStatus::Ok);
        assert_eq!(report.rows.len(), 1);
        assert_eq!(
            report.rows[0].cells.get("job_id"),
            Some(&job_id.to_string())
        );
        assert_eq!(
            report.rows[0].cells.get("job_name"),
            Some(&job_name.to_string())
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "jobs.ready"
                    && diagnostic.severity == DiagnosticSeverity::Info)
        );
    }

    #[test]
    fn run_from_args_renders_jobs_dead_letters_report_for_a_customer_app_runtime_plan() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "jobs".to_string(),
            "dead-letters".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--queue".to_string(),
            "jobs.dead-letter".to_string(),
            "--limit".to_string(),
            "10".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("jobs dead-letters"));
        assert!(rendered.contains("showcase-events"));
        assert!(rendered.contains("dead_letter_id"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn run_from_args_renders_jobs_in_flight_report_for_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "jobs".to_string(),
            "in-flight".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--queue".to_string(),
            "jobs.work".to_string(),
            "--worker-id".to_string(),
            "worker-a".to_string(),
            "--limit".to_string(),
            "10".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("jobs in-flight"));
        assert!(rendered.contains("worker_id"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn run_from_args_renders_live_jobs_in_flight_for_leased_jobs() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture_with_modules(&["cms", "media"]);
        let now_unix_seconds = unix_timestamp_now().unwrap();
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://davenda:test@127.0.0.1:5432/davenda",
            );
        }

        let rendered = {
            let tokio_runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let _runtime_guard = tokio_runtime.enter();
            let built = build_customer_app_runtime_context(&config_path, true).unwrap();
            let Ok(mut host) = built
                .runtime_plan
                .runtime
                .jobs_host("platform-jobs-in-flight-seed")
            else {
                return;
            };
            let definition = host
                .registered_jobs
                .iter()
                .find(|definition| {
                    matches!(
                        definition.contract.trigger,
                        davenda_core::JobTriggerKind::Operator
                            | davenda_core::JobTriggerKind::Webhook
                            | davenda_core::JobTriggerKind::InlineFollowup
                    )
                })
                .cloned()
                .expect("fixture runtime should register a leaseable job");
            let mut request = davenda_runtime::JobDispatchRequest::new(
                definition.contract.name.clone(),
                "cli_in_flight_probe",
            )
            .unwrap();
            if definition.retry_policy.is_retrying() {
                request = request.with_idempotency_key("cli_in_flight_probe").unwrap();
            }
            let Ok(job_id) =
                host.enqueue_job(request, JobInstant::from_unix_seconds(now_unix_seconds))
            else {
                return;
            };
            let Ok(mut leases) = host.lease_ready_jobs(
                &definition.queue,
                "worker-live",
                JobInstant::from_unix_seconds(now_unix_seconds),
                Duration::from_secs(60),
                1,
            ) else {
                return;
            };
            let Some(_lease) = leases.pop() else {
                return;
            };

            let rendered = run_from_args([
                "jobs".to_string(),
                "in-flight".to_string(),
                "--config".to_string(),
                config_path.display().to_string(),
                "--queue".to_string(),
                definition.queue.to_string(),
                "--worker-id".to_string(),
                "worker-live".to_string(),
                "--limit".to_string(),
                "10".to_string(),
            ])
            .unwrap();

            assert!(rendered.contains(job_id.as_str()));
            assert!(rendered.contains("worker-live"));
            assert!(rendered.contains("status: leased"));
            rendered
        };

        unsafe {
            std::env::remove_var("DATABASE_URL");
        }

        assert!(rendered.contains("jobs in-flight"));
    }

    #[test]
    fn run_from_args_executes_jobs_run_against_live_queue_state() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture_with_modules(&["cms", "media"]);
        let now_unix_seconds = unix_timestamp_now().unwrap();
        unsafe {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://davenda:test@127.0.0.1:5432/davenda",
            );
        }

        let (rendered, job_id) = {
            let tokio_runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let _runtime_guard = tokio_runtime.enter();
            let built = build_customer_app_runtime_context(&config_path, true).unwrap();
            let Ok(mut host) = built
                .runtime_plan
                .runtime
                .jobs_host("platform-jobs-run-seed")
            else {
                return;
            };
            let definition = host
                .registered_jobs
                .iter()
                .find(|definition| {
                    matches!(
                        definition.contract.trigger,
                        davenda_core::JobTriggerKind::Operator
                            | davenda_core::JobTriggerKind::Webhook
                            | davenda_core::JobTriggerKind::InlineFollowup
                    )
                })
                .cloned()
                .expect("fixture runtime should register a leaseable job");
            let mut request = davenda_runtime::JobDispatchRequest::new(
                definition.contract.name.clone(),
                "cli_run_probe",
            )
            .unwrap();
            if definition.retry_policy.is_retrying() {
                request = request.with_idempotency_key("cli_run_probe").unwrap();
            }
            let Ok(job_id) =
                host.enqueue_job(request, JobInstant::from_unix_seconds(now_unix_seconds))
            else {
                return;
            };

            let rendered = run_from_args([
                "jobs".to_string(),
                "run".to_string(),
                "--config".to_string(),
                config_path.display().to_string(),
                "--queue".to_string(),
                definition.queue.to_string(),
                "--worker-id".to_string(),
                "worker-live".to_string(),
                "--limit".to_string(),
                "1".to_string(),
            ])
            .unwrap();

            (rendered, job_id)
        };

        unsafe {
            std::env::remove_var("DATABASE_URL");
        }

        assert!(rendered.contains("jobs run"));
        assert!(rendered.contains(job_id.as_str()));
        assert!(
            rendered.contains("status: retried") || rendered.contains("status: dead_lettered"),
            "{rendered}"
        );
    }

    #[test]
    fn run_from_args_requires_confirmation_for_jobs_retry() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "jobs".to_string(),
            "retry".to_string(),
            "dead-letter:job-retry".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`jobs retry` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_warns_when_jobs_retry_cannot_access_live_state() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "jobs".to_string(),
            "retry".to_string(),
            "dead-letter:job-retry".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("jobs retry"));
        assert!(rendered.contains("dead-letter:job-retry"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn run_from_args_requires_confirmation_for_jobs_promote() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "jobs".to_string(),
            "promote".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`jobs promote` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_warns_when_jobs_promote_cannot_access_live_state() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "jobs".to_string(),
            "promote".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("jobs promote"));
        assert!(rendered.contains("DATABASE_URL"));
    }

    #[test]
    fn run_from_args_renders_storage_inspect_for_a_customer_app_runtime_plan() {
        let config_path = customer_app_fixture();

        let rendered = run_from_args([
            "storage".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("storage inspect"));
        assert!(rendered.contains("default_class"));
        assert!(rendered.contains("public_upload"));
        assert!(rendered.contains("cdn_base_url"));
    }

    #[test]
    fn run_from_args_requires_confirmation_for_tls_renew() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "tls".to_string(),
            "renew".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--certificate".to_string(),
            "cert-live".to_string(),
            "--replacement".to_string(),
            "cert-next".to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`tls renew` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_rejects_tls_renew_for_external_termination() {
        ensure_test_tls_material_key();
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "tls".to_string(),
            "renew".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
            "--certificate".to_string(),
            "cert-live".to_string(),
            "--replacement".to_string(),
            "cert-next".to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap_err();

        assert!(
            error.to_string().contains("tls renew is unavailable"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_rejects_tls_validate_challenge_for_external_termination() {
        let config_path = customer_app_fixture();

        let error = run_from_args([
            "tls".to_string(),
            "validate-challenge".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("tls validate-challenge is unavailable"),
            "{}",
            error
        );
    }

    #[test]
    fn run_from_args_renders_tls_validate_challenge_for_cloudflare_origin_customer_app() {
        let _env_lock = database_env_test_lock().lock().unwrap();
        let config_path = customer_app_fixture_with_tls_config(
            "[tls]\nmode = \"cloudflare-origin\"\nprovider = \"cloudflare-origin-ca\"\naccount_secret = { kind = \"env\", var = \"DAVENDA_TLS_VALIDATE_CHALLENGE_SECRET\" }",
        );
        unsafe {
            std::env::set_var(
                "DAVENDA_TLS_VALIDATE_CHALLENGE_SECRET",
                r#"{"cloudflare_api_token":"test-origin-token"}"#,
            );
            std::env::set_var(
                "DAVENDA_TLS_MATERIAL_KEY",
                "tls-validate-challenge-material-key",
            );
        }

        let rendered = run_from_args([
            "tls".to_string(),
            "validate-challenge".to_string(),
            "--config".to_string(),
            config_path.display().to_string(),
        ])
        .unwrap();

        unsafe {
            std::env::remove_var("DAVENDA_TLS_VALIDATE_CHALLENGE_SECRET");
            std::env::remove_var("DAVENDA_TLS_MATERIAL_KEY");
        }

        assert!(rendered.contains("tls validate-challenge"));
        assert!(rendered.contains("cloudflare_origin_ca"));
        assert!(rendered.contains("cloudflare origin-ca credentials resolved"));
    }

    #[test]
    fn run_from_args_renders_storage_verify_policy_report() {
        let fixture = customer_app_fixture_with_assets(false);

        let rendered = run_from_args([
            "storage".to_string(),
            "verify".to_string(),
            "--config".to_string(),
            fixture.config_path.display().to_string(),
            "--policy".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("storage verify"));
        assert!(rendered.contains("public_upload"));
        assert!(rendered.contains("private_shared"));
        assert!(rendered.contains("local_only_sensitive"));
        assert!(rendered.contains("result: ok"));
    }

    #[test]
    fn run_from_args_requires_confirmation_for_assets_publish() {
        let fixture = customer_app_fixture_with_assets(true);

        let error = run_from_args([
            "assets".to_string(),
            "publish".to_string(),
            "--config".to_string(),
            fixture.config_path.display().to_string(),
        ])
        .unwrap_err();

        assert_eq!(error.exit_code(), 2);
        assert!(
            error
                .to_string()
                .contains("`assets publish` requires `--yes` unless `--dry-run` is used")
        );
    }

    #[test]
    fn run_from_args_renders_assets_publish_dry_run_from_customer_app_theme_assets() {
        let fixture = customer_app_fixture_with_assets(true);

        let rendered = run_from_args([
            "assets".to_string(),
            "publish".to_string(),
            "--config".to_string(),
            fixture.config_path.display().to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("assets publish"));
        assert!(rendered.contains("site.css"));
        assert!(rendered.contains("status: planned"));
    }

    #[test]
    fn run_from_args_publishes_assets_into_the_configured_storage_root() {
        let fixture = customer_app_fixture_with_assets(true);

        let rendered = run_from_args([
            "assets".to_string(),
            "publish".to_string(),
            "--config".to_string(),
            fixture.config_path.display().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("Published theme assets"));
        assert!(rendered.contains("status: published"));
        assert!(rendered.contains("storage_path:"));
        assert!(rendered.contains("site.css"));
        assert!(rendered.contains("app.js"));
    }

    #[test]
    fn run_from_args_renders_sample_customer_app_release_doctor_without_ops_blocker() {
        let rendered = run_from_args([
            "release".to_string(),
            "doctor".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("release doctor"));
        assert!(!rendered.contains("module.ops.missing"));
    }

    #[test]
    fn run_from_args_renders_sample_customer_app_module_list_with_ops_installed() {
        let rendered = run_from_args([
            "module".to_string(),
            "list".to_string(),
            "--config".to_string(),
            harbor_shop_platform_config().display().to_string(),
        ])
        .unwrap();

        assert!(rendered.contains("module list"));
        assert!(rendered.contains("ops"));
    }

    #[test]
    fn page_import_mutation_uses_default_locale_and_targets_live_cms_table() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:post:home",
            "target_id": "page:home",
            "checksum": "page-home-v1",
            "normalized": {
                "title": "Home",
                "slug": "home",
                "template": "pages/home",
                "body_html": "<p>Home</p>",
                "publication_state": "published",
                "seo": {
                    "title": "Home",
                    "description": "Landing page",
                    "canonical_path": "/home"
                },
                "media_references": ["asset:hero"]
            }
        });

        let (mutation, persisted) = page_import_mutation(&staged, "en-GB").unwrap();
        let compiled = mutation.compile(1).unwrap();

        assert!(compiled.sql.contains("\"cms_pages\""));
        assert!(compiled.sql.contains("ON CONFLICT (\"page_id\")"));
        assert!(
            compiled
                .bind_values
                .contains(&DataValue::String("en-GB".to_string()))
        );
        assert!(
            compiled
                .bind_values
                .contains(&DataValue::String("/en-GB/home".to_string()))
        );
        assert_eq!(persisted["table"], "cms_pages");
        assert_eq!(persisted["live_path"], "/en-GB/home");
    }

    #[test]
    fn page_import_mutation_rejects_missing_normalized_page_fields() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:post:home",
            "target_id": "page:home",
            "checksum": "page-home-v1",
            "normalized": {
                "slug": "home",
                "publication_state": "published",
                "seo": {}
            }
        });

        let error = page_import_mutation(&staged, "en-GB").unwrap_err();
        assert!(error.to_string().contains("normalized.title"));
    }

    #[test]
    fn event_import_mutation_targets_live_events_catalog_table() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:event:festival",
            "target_id": "event:festival",
            "checksum": "event-festival-v1",
            "normalized": {
                "title": "Festival",
                "slug": "festival",
                "publication_state": "published",
                "starts_at": "2026-06-01T10:00:00Z",
                "summary": "Summer launch",
                "hero_asset": "asset:hero"
            }
        });

        let (mutation, persisted) = event_import_mutation(&staged).unwrap();
        let compiled = mutation.compile(1).unwrap();

        assert!(compiled.sql.contains("\"events_catalog\""));
        assert!(compiled.sql.contains("ON CONFLICT (\"id\")"));
        assert!(
            compiled
                .bind_values
                .contains(&DataValue::String("Festival".to_string()))
        );
        assert_eq!(persisted["table"], "events_catalog");
        assert_eq!(persisted["event_id"], "event:festival");
    }

    #[test]
    fn event_import_mutation_rejects_missing_required_fields() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:event:festival",
            "target_id": "event:festival",
            "checksum": "event-festival-v1",
            "normalized": {
                "slug": "festival",
                "publication_state": "published"
            }
        });

        let error = event_import_mutation(&staged).unwrap_err();
        assert!(error.to_string().contains("normalized.title"));
    }

    #[test]
    fn membership_tier_import_mutation_targets_live_membership_tiers_table() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:tier:gold",
            "target_id": "tier-gold",
            "checksum": "tier-gold-v1",
            "normalized": {
                "title": "Gold",
                "entitlement_key": "membership.gold",
                "interval": "monthly",
                "visibility": "public",
                "status": "active"
            }
        });

        let (mutation, persisted) = membership_tier_import_mutation(&staged).unwrap();
        let compiled = mutation.compile(1).unwrap();

        assert!(compiled.sql.contains("\"membership_tiers\""));
        assert!(compiled.sql.contains("ON CONFLICT (\"id\")"));
        assert!(
            compiled
                .bind_values
                .contains(&DataValue::String("Gold".to_string()))
        );
        assert_eq!(persisted["table"], "membership_tiers");
        assert_eq!(persisted["tier_id"], "tier-gold");
    }

    #[test]
    fn subscription_import_persistence_targets_membership_tables_and_owner_tuples() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:subscription:gold",
            "target_id": "sub-gold",
            "checksum": "subscription-gold-v1",
            "normalized": {
                "tier_id": "tier-gold",
                "principal_id": "alice",
                "status": "active",
                "entitlement_key": "membership.gold",
                "entitlement_id": "entitlement:sub-gold",
                "active": true,
                "renews_at": 1770000000
            }
        });

        let (mutations, updates, persisted) =
            subscription_import_persistence(&staged, "showcase-events").unwrap();

        assert_eq!(mutations.len(), 2);
        let compiled_subscription = mutations[0].compile(1).unwrap();
        let compiled_entitlement = mutations[1].compile(1).unwrap();
        assert!(
            compiled_subscription
                .sql
                .contains("\"membership_subscriptions\"")
        );
        assert!(
            compiled_entitlement
                .sql
                .contains("\"membership_entitlements\"")
        );
        assert_eq!(updates.len(), 2);
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::subscription("sub-gold"),
                Relation::Owner,
                DefaultSubject::entity(Entity::user("alice")),
            )))
        );
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::subscription("sub-gold"),
                Relation::Storefront,
                DefaultSubject::entity(Entity::storefront("showcase-events")),
            )))
        );
        assert_eq!(persisted.as_array().unwrap().len(), 3);
        assert_eq!(persisted[0]["table"], "membership_subscriptions");
        assert_eq!(persisted[1]["table"], "membership_entitlements");
        assert_eq!(persisted[2]["table"], "auth_tuples");
    }

    #[test]
    fn membership_subscription_import_transaction_batches_account_and_membership_writes() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:subscription:gold",
            "target_id": "sub-gold",
            "checksum": "subscription-gold-v1",
            "normalized": {
                "principal_id": "alice",
                "email": "alice@example.com",
                "username": "alice",
                "display_name": "Alice Example",
                "status": "active",
                "tier_id": "tier-gold",
                "entitlement_key": "membership.gold",
                "entitlement_id": "entitlement:sub-gold",
                "active": true,
                "renews_at": 1770000000
            }
        });

        let runtime = DataRuntime::from_config(&DatabaseConfig::default()).unwrap();
        let (account_mutation, _) =
            subscription_member_account_bootstrap_mutation(&staged).unwrap();
        let (mutations, _, _) =
            subscription_import_persistence(&staged, "showcase-events").unwrap();
        let compiled = compile_membership_import_transaction(
            &runtime,
            "import.membership.subscription",
            &[
                ("membership_member_accounts", "upsert"),
                ("membership_subscriptions", "upsert"),
                ("membership_entitlements", "upsert"),
            ],
            std::iter::once(account_mutation).chain(mutations).collect(),
        )
        .unwrap();

        assert_eq!(
            compiled.statements[0].sql,
            "SET TRANSACTION ISOLATION LEVEL SERIALIZABLE"
        );
        assert!(
            compiled
                .statements
                .iter()
                .any(|statement| statement.sql.contains("\"membership_member_accounts\""))
        );
        assert!(
            compiled
                .statements
                .iter()
                .any(|statement| statement.sql.contains("\"membership_subscriptions\""))
        );
        assert!(
            compiled
                .statements
                .iter()
                .any(|statement| statement.sql.contains("\"membership_entitlements\""))
        );
    }

    #[test]
    fn subscription_member_account_bootstrap_statement_targets_member_accounts_table() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:subscription:gold",
            "target_id": "sub-gold",
            "checksum": "subscription-gold-v1",
            "normalized": {
                "principal_id": "alice",
                "email": "alice@example.com",
                "username": "alice",
                "display_name": "Alice Example",
                "status": "active",
                "tier_id": "tier-gold",
                "entitlement_key": "membership.gold",
                "entitlement_id": "entitlement:sub-gold",
                "active": true,
                "renews_at": 1770000000
            }
        });

        let (statement, persisted) =
            subscription_member_account_bootstrap_statement(&staged).unwrap();

        assert!(statement.sql.contains("\"membership_member_accounts\""));
        assert!(statement.sql.contains("ON CONFLICT (\"id\") DO NOTHING"));
        assert!(
            statement
                .bind_values
                .contains(&DataValue::String("alice@example.com".to_string()))
        );
        assert_eq!(persisted["table"], "membership_member_accounts");
        assert_eq!(persisted["member_account_id"], "alice");
        assert_eq!(persisted["email"], "alice@example.com");
        assert_eq!(persisted["display_name"], "Alice Example");
        assert_eq!(persisted["bootstrap"], "subscription");
    }

    #[test]
    fn user_account_import_mutation_targets_live_member_account_table() {
        let staged = serde_json::json!({
            "source_system": "wordpress",
            "source_key": "wp:user:alice",
            "target_id": "alice",
            "checksum": "user-alice-v1",
            "normalized": {
                "principal_id": "alice",
                "email": "alice@example.com",
                "username": "alice",
                "display_name": "Alice Example",
                "legacy_roles": ["administrator"]
            }
        });

        let (mutation, persisted) = user_account_import_mutation(&staged).unwrap();
        let compiled = mutation.compile(1).unwrap();

        assert!(compiled.sql.contains("\"membership_member_accounts\""));
        assert!(compiled.sql.contains("ON CONFLICT (\"id\")"));
        assert_eq!(persisted["table"], "membership_member_accounts");
        assert_eq!(persisted["member_account_id"], "alice");
        assert_eq!(persisted["email"], "alice@example.com");
        assert_eq!(persisted["display_name"], "Alice Example");
    }

    fn test_import_auth_mapping(markdown: &str) -> ImportAuthMapping {
        ImportAuthMapping::from_markdown_str(markdown).expect("auth mapping should parse")
    }

    #[test]
    fn user_import_updates_parse_auth_mapping_markdown_entries() {
        let mapping = test_import_auth_mapping(
            "# Auth Mapping\n\n- `administrator` -> `cms.page.publish`, `asset.publish`, `events.booking.manage`\n- `shop_manager` -> `events.booking.manage`\n",
        );

        assert_eq!(
            mapping
                .capabilities_for_role("administrator")
                .unwrap()
                .to_vec(),
            vec![
                "cms.page.publish".to_string(),
                "asset.publish".to_string(),
                "events.booking.manage".to_string(),
            ]
        );
        assert_eq!(
            mapping
                .capabilities_for_role("shop_manager")
                .unwrap()
                .to_vec(),
            vec!["events.booking.manage".to_string()]
        );
    }

    #[test]
    fn validate_import_auth_mapping_rejects_unsupported_capabilities_before_live_import() {
        let mapping = ImportAuthMapping::new()
            .with_role_capabilities("legacy-ops", ["system.cluster.rotate"])
            .unwrap();
        let auth_package = configured_auth_model_package("platform-default-auth");

        let error = validate_import_auth_mapping(&mapping, &auth_package).unwrap_err();

        assert!(error.to_string().contains("legacy-ops"));
        assert!(error.to_string().contains("system.cluster.rotate"));
        assert!(error.to_string().contains("unsupported capability"));
    }

    #[test]
    fn user_import_updates_map_administrators_into_group_and_site_admin_tuples_from_auth_mapping() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["administrator"]
            }
        });
        let auth_mapping = test_import_auth_mapping(
            "- `administrator` -> `cms.page.publish`, `asset.publish`, `events.booking.manage`\n",
        );
        let auth_package = configured_auth_model_package("platform-default-auth");

        let (updates, persisted) = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(persisted["table"], "auth_tuples");
        assert_eq!(persisted["principal_id"], "alice");
        assert_eq!(persisted["site_id"], "main");
        assert_eq!(persisted["storefront_id"], "shoppr");
        assert_eq!(persisted["writes"], 2);
        assert_eq!(
            persisted["mapped_capabilities"]["administrator"],
            serde_json::json!(["cms.page.publish", "asset.publish", "events.booking.manage"])
        );
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::group("legacy-role:administrator"),
                Relation::Member,
                DefaultSubject::entity(Entity::user("alice")),
            )))
        );
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::site("main"),
                Relation::Admin,
                DefaultSubject::userset(
                    Entity::group("legacy-role:administrator"),
                    Relation::Member
                ),
            )))
        );
    }

    #[test]
    fn user_import_updates_map_editors_into_group_and_site_editor_tuples_from_auth_mapping() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["editor"]
            }
        });
        let auth_mapping =
            test_import_auth_mapping("- `editor` -> `cms.page.publish`, `asset.publish`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let (updates, persisted) = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(persisted["roles"], serde_json::json!(["editor"]));
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::site("main"),
                Relation::Editor,
                DefaultSubject::userset(Entity::group("legacy-role:editor"), Relation::Member),
            )))
        );
    }

    #[test]
    fn user_import_updates_map_customer_into_storefront_member_without_site_scope() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["customer"]
            }
        });
        let auth_mapping = test_import_auth_mapping("- `customer` -> `checkout.session.create`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let (updates, persisted) =
            user_import_updates(&staged, None, "shoppr", &auth_package, &auth_mapping)
                .unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(persisted["site_id"], serde_json::Value::Null);
        assert_eq!(persisted["roles"], serde_json::json!(["member"]));
        assert_eq!(
            persisted["mapped_capabilities"]["customer"],
            serde_json::json!(["checkout.session.create"])
        );
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::storefront("shoppr"),
                Relation::Member,
                DefaultSubject::userset(Entity::group("legacy-role:customer"), Relation::Member),
            )))
        );
    }

    #[test]
    fn user_import_updates_map_shop_manager_into_site_admin_from_auth_mapping() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["shop_manager"]
            }
        });
        let auth_mapping =
            test_import_auth_mapping("- `shop_manager` -> `events.booking.manage`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let (updates, persisted) = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(persisted["roles"], serde_json::json!(["admin"]));
        assert_eq!(
            persisted["mapped_capabilities"]["shop_manager"],
            serde_json::json!(["events.booking.manage"])
        );
        assert!(
            updates.contains(&DefaultTupleUpdate::Write(DefaultTuple::new(
                Entity::site("main"),
                Relation::Admin,
                DefaultSubject::userset(
                    Entity::group("legacy-role:shop_manager"),
                    Relation::Member
                ),
            )))
        );
    }

    #[test]
    fn user_import_updates_allows_empty_legacy_roles_without_live_auth_grants() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": []
            }
        });
        let auth_mapping = test_import_auth_mapping("- `administrator` -> `cms.page.publish`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let (updates, persisted) = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap();

        assert!(updates.is_empty());
        assert_eq!(persisted["legacy_roles"], serde_json::json!([]));
        assert_eq!(persisted["roles"], serde_json::json!([]));
        assert_eq!(persisted["writes"], 0);
    }

    #[test]
    fn user_import_updates_rejects_legacy_roles_missing_from_auth_mapping() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["seo_manager"]
            }
        });
        let auth_mapping = test_import_auth_mapping("- `administrator` -> `cms.page.publish`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let error = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap_err();
        assert!(error.to_string().contains("seo_manager"));
        assert!(error.to_string().contains("auth mapping"));
    }

    #[test]
    fn user_import_updates_rejects_site_scoped_capabilities_without_site_context() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["editor"]
            }
        });
        let auth_mapping =
            test_import_auth_mapping("- `editor` -> `cms.page.publish`, `asset.publish`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let error = user_import_updates(&staged, None, "shoppr", &auth_package, &auth_mapping)
            .unwrap_err();

        assert!(error.to_string().contains("non-empty `site`"));
    }

    #[test]
    fn user_import_updates_reject_missing_required_fields() {
        let staged = serde_json::json!({
            "normalized": {
                "legacy_roles": ["administrator"]
            }
        });
        let auth_mapping = test_import_auth_mapping("- `administrator` -> `cms.page.publish`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let error = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap_err();
        assert!(error.to_string().contains("normalized.principal_id"));
    }

    #[test]
    fn user_import_updates_reject_tenant_scoped_capabilities_from_auth_mapping() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["ops_admin"]
            }
        });
        let auth_mapping = test_import_auth_mapping("- `ops_admin` -> `system.config.write`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let error = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap_err();
        assert!(error.to_string().contains("tenant-scoped auth"));
    }

    #[test]
    fn user_import_updates_rejects_read_public_capabilities_without_resource_specific_tuples() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["media_guest"]
            }
        });
        let auth_mapping = test_import_auth_mapping("- `media_guest` -> `asset.read_public`\n");
        let auth_package = configured_auth_model_package("platform-default-auth");

        let error = user_import_updates(
            &staged,
            Some("main"),
            "shoppr",
            &auth_package,
            &auth_mapping,
        )
        .unwrap_err();

        assert!(error.to_string().contains("read_public"));
        assert!(error.to_string().contains("cannot be granted safely"));
    }
}
