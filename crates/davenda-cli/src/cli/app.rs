use crate::CliModelError;
use crate::cli::args::{
    AssetsPublishInvocation, AuthCheckInvocation, AuthPackageValidateInvocation,
    CacheWarmInvocation, CliInput, DevServerInvocation, JobsStatusInvocation,
    MigrateApplyInvocation, TlsRenewInvocation, parse,
};
use crate::cli::auth::AuthExplainResult;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::customer_app::{load_customer_app_context, load_official_modules};
use crate::cli::error::CliRunError;
use crate::cli::import::{ImportCutoverInvocation, ImportRunInvocation};
use crate::cli::render::{render_auth_explain, render_command_report};
use crate::registry::CliRuntime;
use crate::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};
use davenda_app::{CustomerAppManifest, CustomerAppRuntimePlan};
use davenda_assets::{AssetDeliveryTarget, ContentFingerprint, FingerprintAlgorithm, RevisionId};
use davenda_auth::{
    AuthModelPackage, DavendaAuth, DefaultSubject, DefaultTuple, DefaultTupleUpdate, Entity,
    Relation, configured_auth_model_package,
};
use davenda_cache::CacheInstant;
use davenda_commerce::EntitlementKey;
use davenda_config::{PlatformConfig, StorageClass};
use davenda_core::validate_module_capabilities;
use davenda_data::{
    DataRuntime, DataValue, MigrationPlan, MigrationRegistry, MutationAction, MutationSpec,
    PostgresDataClient,
};
use davenda_import::{
    CutoverCheck, CutoverExecutionJournal, CutoverPlan, CutoverStepRecord, ImportManifest,
    ImportModelError, PublicationMode, RollbackTrigger,
};
use davenda_memberships::{
    BillingInterval, MemberAccountId, MembershipTierId, SubscriptionId, SubscriptionStatus,
    TierVisibility,
};
use davenda_runtime::{
    CacheDisposition, EnvironmentSecretResolver, HandlerResponse, HttpMethod, RequestInput,
    RuntimeBuilder, StorageHost,
};
use davenda_storage::{
    StorageDeliveryLocation, StoragePlanRequest, StoragePolicy, StoragePolicyOverride,
};
use davenda_tls::{CertificateId, TlsInstant};
use reqwest::Url;
use reqwest::blocking::Client as BlockingHttpClient;
use reqwest::redirect::Policy as RedirectPolicy;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
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
            let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
                CliRunError::execution(format!(
                    "failed to load platform config from `{}`: {error}",
                    invocation.config_path.display()
                ))
            })?;
            let backend = LiveAuthExplainBackend::from_config(&config)?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to start the CLI async runtime: {error}"
                    ))
                })?;
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
        CliInput::AuthPackageValidate {
            output_mode,
            invocation,
        } => {
            let report = run_auth_package_validate(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::ModuleList {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package = configured_auth_model_package(context.config.auth.package.clone());
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
        CliInput::MigratePlan {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package = configured_auth_model_package(context.config.auth.package.clone());
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
            let context = load_customer_app_context(&config_path)?;
            let auth_package = configured_auth_model_package(context.config.auth.package.clone());
            let report = context
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
                })?;
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
        CliInput::JobsStatus {
            output_mode,
            invocation,
        } => {
            let report = run_jobs_status(&invocation)?;
            render_command_report(&report, output_mode)
        }
        CliInput::TlsStatus {
            output_mode,
            config_path,
        } => {
            let report = run_tls_status(&config_path)?;
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
            invocation,
        } => {
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
        "  platform auth explain [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "  platform auth package validate [--config <path>] [--json]",
        "  platform module list [--config <path>] [--json]",
        "  platform migrate plan [--config <path>] [--json]",
        "  platform migrate apply [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform release doctor [--config <path>] [--json]",
        "  platform cache warm [--config <path>] --scope public --route <path> [--route <path> ...] [--dry-run] [--json]",
        "  platform jobs status [--config <path>] [--queue <name>] [--json]",
        "  platform tls status [--config <path>] [--json]",
        "  platform tls renew [--config <path>] --certificate <id> --replacement <id> [--dry-run] [--yes] [--json]",
        "  platform storage verify [--config <path>] [--policy] [--json]",
        "  platform assets publish [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform import run <manifest-path> [--dry-run] [--json]",
        "  platform import cutover <manifest-path> [--apply] [--yes] [--legacy-freeze-confirmed] [--json]",
        "  platform import cutover <manifest-path> --switch --base-url <url> --yes [--json]",
        "  platform import cutover <manifest-path> --observe --base-url <url> --yes [--json]",
        "  platform import cutover <manifest-path> --rollback --base-url <url> --reason <text> --yes [--json]",
        "",
        "Examples:",
        "  platform dev server --config config/platform.toml",
        "  platform config validate --config config/platform.toml",
        "  platform auth check --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform auth package validate --config config/platform.toml",
        "  platform module list --config config/platform.toml",
        "  platform migrate plan --config config/platform.toml",
        "  platform migrate apply --config config/platform.toml --dry-run",
        "  platform release doctor --config config/platform.toml",
        "  platform cache warm --config config/platform.toml --scope public --route /en-GB/home",
        "  platform jobs status --config config/platform.toml",
        "  platform tls status --config config/platform.toml",
        "  platform tls renew --config config/platform.toml --certificate cert-live --replacement cert-next --dry-run",
        "  platform storage verify --config config/platform.toml --policy",
        "  platform assets publish --config apps/harbor-shop/platform.toml --dry-run",
        "  platform import run imports/wordpress-events.toml",
        "  platform import run imports/wordpress-events.toml --dry-run",
        "  platform import cutover imports/wordpress-events.toml",
        "  platform import cutover imports/wordpress-events.toml --apply --yes --legacy-freeze-confirmed",
        "  platform import cutover imports/wordpress-events.toml --switch --base-url https://shop.example.com --yes",
        "  platform import cutover imports/wordpress-events.toml --observe --base-url https://shop.example.com --yes",
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
}

fn run_auth_check(invocation: &AuthCheckInvocation) -> Result<CommandReport, CliRunError> {
    let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load platform config from `{}`: {error}",
            invocation.config_path.display()
        ))
    })?;
    let data = DataRuntime::from_config(&config.database).map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live auth check backend: {error}"
        ))
    })?;
    let client = data.connect_lazy_postgres().map_err(|error| {
        CliRunError::execution(format!(
            "failed to initialize the live auth check backend: {error}"
        ))
    })?;
    let package = configured_auth_model_package(config.auth.package.clone());
    let binding = package
        .resolve_binding(invocation.capability, &invocation.resource)
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to resolve capability binding for auth check: {error}"
            ))
        })?;
    let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
    let auth = DavendaAuth::new(engine, config.auth.tenant_id);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            CliRunError::execution(format!("failed to start the CLI async runtime: {error}"))
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

fn run_auth_package_validate(
    invocation: &AuthPackageValidateInvocation,
) -> Result<CommandReport, CliRunError> {
    let context = load_customer_app_context(&invocation.config_path)?;
    let auth_package = configured_auth_model_package(context.config.auth.package.clone());
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
    let advisory_migration_entries = count_advisory_migration_entries(&built.runtime_plan);

    if dry_run {
        return build_migrate_apply_report(
            &built.manifest,
            executable_plan,
            None,
            None,
            advisory_migration_entries,
            true,
        );
    }

    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;
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
        advisory_migration_entries,
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
        Some(
            built
                .runtime_plan
                .runtime
                .jobs_host("platform-jobs-status")
                .map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to build jobs status host for `{}`: {error}",
                        built.manifest.id
                    ))
                })?,
        )
    } else {
        None
    };
    let coordinator_state = jobs_host.as_ref().map(|host| {
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
    if let Some(host) = jobs_host {
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
    let host = built.runtime_plan.runtime.tls_host().map_err(|error| {
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
            && plan
                .ordered_importers
                .iter()
                .any(|importer| {
                    matches!(
                        importer.resource_kind.as_str(),
                        "page" | "event" | "membership_tier" | "subscription"
                    )
                });
        let requires_live_auth = publish_validated
            && plan
                .ordered_importers
                .iter()
                .any(|importer| matches!(importer.resource_kind.as_str(), "user" | "subscription"));
        let requires_live_site_auth = publish_validated
            && plan
                .ordered_importers
                .iter()
                .any(|importer| importer.resource_kind == "user");
        if requires_live_site_auth && manifest.site.is_none() {
            return Err(CliRunError::execution(format!(
                "publish-validated import manifest `{}` requires `site` to materialize live auth state",
                invocation.manifest_path.display()
            )));
        }
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
            (true, Some(tokio_runtime)) => {
                Some(build_import_auth_context(runtime, &manifest, tokio_runtime)?)
            }
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
                        let client =
                            ensure_import_data_client(&data_runtime, &mut data_client)?;
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
                        let client =
                            ensure_import_data_client(&data_runtime, &mut data_client)?;
                        materialize_event_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &client,
                            staged_record,
                        )?;
                    }
                    "membership_tier" if publish_validated => {
                        let client =
                            ensure_import_data_client(&data_runtime, &mut data_client)?;
                        materialize_membership_tier_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &client,
                            staged_record,
                        )?;
                    }
                    "subscription" if publish_validated => {
                        let client =
                            ensure_import_data_client(&data_runtime, &mut data_client)?;
                        let auth_context = auth_context.as_mut().expect(
                            "publish-validated subscription imports build a live auth context",
                        );
                        materialize_subscription_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
                            &client,
                            auth_context,
                            staged_record,
                        )?;
                    }
                    "user" if publish_validated => {
                        let auth_context = auth_context
                            .as_mut()
                            .expect("publish-validated imports build a live auth context");
                        materialize_user_record(
                            tokio_runtime
                                .as_ref()
                                .expect("publish-validated imports build a runtime"),
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
    report: CommandReport,
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
    if !invocation.confirmed {
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

    let switched_at = unix_timestamp_now()?;
    run_cutover_step(
        &mut journal,
        &journal_path,
        &evaluated.manifest.run_id,
        "switch.confirmed",
        || Ok(format!("operator confirmed live switch to `{base_url}`")),
    )?;
    journal.confirm_switch(base_url.clone(), switched_at);
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;

    let mut report = journal.command_report().map_err(|error| {
        CliRunError::execution(format!(
            "failed to render cutover switch report for `{}`: {error}",
            evaluated.manifest.run_id
        ))
    })?;
    report.summary = format!(
        "Cutover switch for import run `{}` is confirmed against `{}`",
        evaluated.manifest.run_id, base_url
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.switch",
        format!("live switch confirmed against `{base_url}`"),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.journal",
        format!("cutover journal persisted at `{}`", journal_path.display()),
    )?;
    Ok(report)
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
    route_resolution: bool,
    canonical_urls: bool,
    media_reachability: bool,
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
        .map(build_observation_verification_checks)
        .transpose()?
        .unwrap_or_default();
    if sample_routes.is_empty() {
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
    save_cutover_journal(&journal, &journal_path, &evaluated.manifest.run_id)?;

    let client = build_cutover_probe_client()?;
    let probe =
        execute_cutover_observation_probe(&client, base_url, &sample_routes, verification_checks)?;

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

    let observation_started_at = journal
        .observation_started_at_unix_seconds
        .unwrap_or(probe_time);
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

    let rolled_back_at = unix_timestamp_now()?;
    run_cutover_step(
        &mut journal,
        &journal_path,
        &evaluated.manifest.run_id,
        "rollback.executed",
        || {
            Ok(format!(
                "operator rolled traffic back from `{base_url}`: {reason}"
            ))
        },
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
        "Cutover rollback for import run `{}` is recorded against `{}`",
        evaluated.manifest.run_id, base_url
    );
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Warning,
        "cutover.rollback",
        format!("rollback confirmed for `{base_url}`: {reason}"),
    )?;
    push_report_diagnostic(
        &mut report,
        DiagnosticSeverity::Info,
        "cutover.journal",
        format!("cutover journal persisted at `{}`", journal_path.display()),
    )?;
    Ok(report)
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
            evaluate_verification_readiness(verification);
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
            evaluate_cutover_migration_readiness(&runtime.built)?;
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "migrate.apply",
            migrations_detail,
            true,
            migrations_ready,
        )?);
    }

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
        report,
    })
}

fn cutover_preflight_ready(plan: &CutoverPlan) -> bool {
    plan.checks.iter().all(|check| {
        if !check.required {
            return true;
        }
        match check.id.as_str() {
            "import.package" | "target.runtime" | "final.import.mode" | "release.doctor" => {
                check.satisfied
            }
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
                "Pending executable migrations were applied to the target runtime",
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

fn evaluate_verification_readiness(
    verification: &davenda_import::ImportVerification,
) -> (bool, String) {
    match build_observation_verification_checks(verification) {
        Ok(checks) => (
            true,
            format!(
                "verification checks supported: {}",
                render_supported_verification_checks(verification, checks)
            ),
        ),
        Err(error) => (false, error.to_string()),
    }
}

fn build_observation_verification_checks(
    verification: &davenda_import::ImportVerification,
) -> Result<ObservationVerificationChecks, CliRunError> {
    let mut checks = ObservationVerificationChecks::default();
    for required in &verification.required {
        match required.as_str() {
            "record_counts" => {}
            "route_resolution" => checks.route_resolution = true,
            "canonical_urls" => checks.canonical_urls = true,
            "media_reachability" => checks.media_reachability = true,
            other => {
                return Err(CliRunError::execution(format!(
                    "verification check `{other}` is not yet supported by cutover observation"
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

    Ok(checks)
}

fn render_supported_verification_checks(
    verification: &davenda_import::ImportVerification,
    checks: ObservationVerificationChecks,
) -> String {
    let mut rendered = Vec::new();
    if verification
        .required
        .iter()
        .any(|check| check == "record_counts")
    {
        rendered.push("record_counts(import-run)");
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
    rendered.join(", ")
}

fn execute_cutover_observation_probe(
    client: &BlockingHttpClient,
    base_url: &str,
    sample_routes: &[String],
    verification_checks: ObservationVerificationChecks,
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
        let body = response.text().map_err(|error| {
            CliRunError::execution(format!(
                "failed to read cutover route `{}` at `{}`: {error}",
                route, url
            ))
        })?;
        let outcome = if (200..400).contains(&status_code) {
            let mut outcome = vec!["healthy".to_string()];
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
            outcome.join(" ")
        } else {
            failures.push(format!(
                "route `{route}` returned unexpected status {} during live observation",
                status_code
            ));
            "unexpected_status".to_string()
        };
        routes.push(ObservedCutoverRoute {
            route: route.clone(),
            status_code,
            outcome,
        });
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
            configured_auth_model_package(context.config.auth.package.clone()),
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

fn evaluate_cutover_migration_readiness(
    built: &BuiltCustomerAppContext,
) -> Result<(bool, String), CliRunError> {
    let executable_plan = &built.runtime_plan.runtime.install_migrations;
    let advisory = count_advisory_migration_entries(&built.runtime_plan);
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
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;
    let applied_keys = tokio_runtime
        .block_on(async { client.applied_migration_keys().await })
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to read applied migrations for `{}`: {error}",
                built.manifest.id
            ))
        })?;
    let pending_plan = pending_migration_plan(executable_plan, &applied_keys)?;
    let pending_steps = pending_plan.ordered_steps().len();
    let ready = pending_steps == 0 && advisory == 0;
    let detail = if ready {
        format!(
            "no pending executable or advisory migration work remains for `{}`",
            built.manifest.id
        )
    } else {
        format!(
            "{} pending executable migration steps and {} advisory migration entries remain for `{}`",
            pending_steps, advisory, built.manifest.id
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
    advisory_migration_entries: usize,
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
    if advisory_migration_entries > 0 {
        push_report_diagnostic(
            &mut report,
            DiagnosticSeverity::Warning,
            "migrate.steps.advisory_only",
            format!(
                "{} auth-package or customer-app migration steps remain advisory only because they do not yet compile into executable SQL batches",
                advisory_migration_entries
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

fn count_advisory_migration_entries(runtime_plan: &CustomerAppRuntimePlan) -> usize {
    runtime_plan
        .migration_summary
        .entries()
        .iter()
        .filter(|entry| entry.step_id.is_none())
        .count()
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
    let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load platform config from `{}`: {error}",
            invocation.config_path.display()
        ))
    })?;

    let cookie_secret = read_runtime_secret("DAVENDA_COOKIE_SECRET")?;
    let csrf_secret = read_runtime_secret("DAVENDA_CSRF_SECRET")?;
    let bind = config.server.bind.clone();
    let auth_package_name = config.auth.package.clone();
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| CliRunError::execution(format!("failed to start runtime: {error}")))?;

    tokio_runtime.block_on(async move {
        let modules = load_official_modules(&config)?;
        let builder = RuntimeBuilder::new(
            config.clone(),
            configured_auth_model_package(auth_package_name),
        );
        let mut builder = builder;
        for module in modules {
            builder = builder.with_boxed_module(module);
        }
        let plan = builder.build().map_err(|error| {
            CliRunError::execution(format!("failed to build runtime plan: {error}"))
        })?;
        let server = plan
            .server_host(
                &EnvironmentSecretResolver,
                cookie_secret.as_bytes(),
                csrf_secret.as_bytes(),
            )
            .map_err(|error| {
                CliRunError::execution(format!("failed to build dev server host: {error}"))
            })?;
        let listener = tokio::net::TcpListener::bind(&bind)
            .await
            .map_err(|error| {
                CliRunError::execution(format!("failed to bind dev server on `{bind}`: {error}"))
            })?;

        println!("Serving `{}` on http://{bind}", plan.config.app.name);
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
    manifest: &ImportManifest,
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
    let auth_package = configured_auth_model_package(auth_package_name.clone());
    tokio_runtime
        .block_on(async { auth.apply_model_package(&auth_package).await })
        .map_err(|error| {
            CliRunError::execution(format!(
                "failed to apply auth model package `{}` for live import: {error}",
                auth_package_name
            ))
        })?;

    Ok(LiveImportAuthContext {
        auth,
        site_id: manifest.site.clone(),
        storefront_id: runtime.built.manifest.id.to_string(),
    })
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

        for table in records.iter().filter_map(|record| {
            record
                .get("normalized")
                .and_then(Value::as_object)
                .and_then(|normalized| normalized.get("persisted"))
                .and_then(Value::as_object)
                .and_then(|persisted| persisted.get("table"))
                .and_then(Value::as_str)
        }) {
            *user_counts.entry(table.to_string()).or_insert(0) += 1;
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
        .execute_write(revision.storage_plan(), &bytes)
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
    auth_context: &LiveImportAuthContext,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (updates, persisted) = user_import_updates(staged_record, auth_context.site_id.as_deref())?;
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
    normalized.insert("persisted".to_string(), persisted);
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
    data_client: &PostgresDataClient,
    auth_context: &LiveImportAuthContext,
    staged_record: &mut Value,
) -> Result<(), ImportModelError> {
    let (mutations, auth_updates, persisted) =
        subscription_import_persistence(staged_record, &auth_context.storefront_id)?;
    for mutation in mutations {
        let statement = mutation.compile(1).map_err(import_data_model_error)?;
        tokio_runtime
            .block_on(async { data_client.execute_statement(&statement).await })
            .map_err(|error| ImportModelError::ManifestParse {
                message: format!("failed to persist imported subscription state: {error}"),
            })?;
    }
    tokio_runtime
        .block_on(async { auth_context.auth.write(auth_updates).await })
        .map_err(|error| ImportModelError::ManifestParse {
            message: format!("failed to persist imported subscription auth state: {error}"),
        })?;

    let normalized = staged_record
        .get_mut("normalized")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged subscription record is missing `normalized` object data".to_string(),
        })?;
    normalized.insert("persisted".to_string(), persisted);
    Ok(())
}

fn user_import_updates(
    staged_record: &Value,
    site_id: Option<&str>,
) -> Result<(Vec<DefaultTupleUpdate>, Value), ImportModelError> {
    let normalized = staged_record
        .get("normalized")
        .and_then(Value::as_object)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged user record is missing `normalized` object data".to_string(),
        })?;
    let principal_id = required_normalized_string(normalized, "principal_id")?;
    let site_id = site_id.ok_or_else(|| ImportModelError::ManifestParse {
        message: "live user import requires a non-empty `site`".to_string(),
    })?;
    if site_id.is_empty() {
        return Err(ImportModelError::ManifestParse {
            message: "live user import requires a non-empty `site`".to_string(),
        });
    }
    let legacy_roles = normalized
        .get("legacy_roles")
        .and_then(Value::as_array)
        .ok_or_else(|| ImportModelError::ManifestParse {
            message: "staged user record is missing `normalized.legacy_roles`".to_string(),
        })?;
    if legacy_roles.is_empty() {
        return Err(ImportModelError::ManifestParse {
            message: "live user import requires at least one `legacy_roles` entry".to_string(),
        });
    }

    let user = DefaultSubject::entity(Entity::user(principal_id.clone()));
    let site = Entity::site(site_id.to_string());
    let mut updates = Vec::new();
    let mut effective_roles = Vec::new();

    for role in legacy_roles {
        let role = role
            .as_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ImportModelError::ManifestParse {
                message: "staged user record has a non-string `normalized.legacy_roles` entry"
                    .to_string(),
            })?;
        let group = Entity::group(format!("legacy-role:{role}"));
        updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
            group.clone(),
            Relation::Member,
            user.clone(),
        )));
        match role {
            "administrator" => {
                updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                    site.clone(),
                    Relation::Admin,
                    DefaultSubject::userset(group, Relation::Member),
                )));
                effective_roles.push(role.to_string());
            }
            "editor" => {
                updates.push(DefaultTupleUpdate::Write(DefaultTuple::new(
                    site.clone(),
                    Relation::Editor,
                    DefaultSubject::userset(group, Relation::Member),
                )));
                effective_roles.push(role.to_string());
            }
            unsupported => {
                return Err(ImportModelError::ManifestParse {
                    message: format!(
                        "legacy role `{unsupported}` cannot be mapped safely into the shipped auth model yet"
                    ),
                });
            }
        }
    }

    Ok((
        updates.clone(),
        serde_json::json!({
            "table": "auth_tuples",
            "principal_id": principal_id,
            "site_id": site_id,
            "roles": effective_roles,
            "writes": updates.len(),
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

fn render_entity(entity: &Entity) -> String {
    format!("{}:{}", entity.namespace().as_str(), entity.id())
}

fn render_subject(subject: &DefaultSubject) -> String {
    match subject {
        DefaultSubject::Entity(entity) => render_entity(entity),
        DefaultSubject::Userset { object, relation } => {
            format!("{}#{}", render_entity(object), relation.as_str())
        }
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
    }

    impl LiveProbeResponse {
        fn html(status_code: u16, body: impl Into<String>) -> Self {
            Self {
                status_code,
                content_type: "text/html; charset=utf-8",
                body: body.into().into_bytes(),
            }
        }

        fn binary(status_code: u16, body: Vec<u8>) -> Self {
            Self {
                status_code,
                content_type: "application/octet-stream",
                body,
            }
        }
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
        let path = request_line
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .split('?')
            .next()
            .unwrap_or("/");

        let (status, content_type, response_body) = match path {
            "/health" => (
                "200 OK",
                "application/json",
                serde_json::json!({
                    "status": health_status,
                    "maintenance": { "enabled": maintenance_enabled }
                })
                .to_string()
                .into_bytes(),
            ),
            "/ready" | "/readiness" => (
                "200 OK",
                "application/json",
                serde_json::json!({ "status": readiness_status })
                    .to_string()
                    .into_bytes(),
            ),
            route => {
                let response = routes.get(route).cloned().unwrap_or_else(|| {
                    LiveProbeResponse::html(404, format!("<html><body>{route}</body></html>"))
                });
                let status = match response.status_code {
                    200 => "200 OK",
                    302 => "302 Found",
                    304 => "304 Not Modified",
                    500 => "500 Internal Server Error",
                    _ => "404 Not Found",
                };
                (status, response.content_type, response.body)
            }
        };

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("davenda-cli-workflow-{suffix}"));
        let config_dir = root.join("config");
        let app_root = root.join("apps").join("showcase-events");
        let templates_root = app_root.join("templates").join("pages");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&templates_root).unwrap();
        fs::write(config_dir.join("platform.toml"), DISABLED_EXPLAIN_CONFIG).unwrap();
        fs::write(app_root.join("app.toml"), CUSTOMER_APP_MANIFEST).unwrap();
        fs::write(
            templates_root.join("home.html"),
            "<html><body><main>Showcase Events</main></body></html>",
        )
        .unwrap();

        config_dir.join("platform.toml")
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
        write_cutover_observe_manifest_with_checks(
            fixture,
            name,
            observation_window_minutes,
            &["record_counts"],
        )
    }

    fn write_cutover_observe_manifest_with_checks(
        fixture: &ImportFixture,
        name: &str,
        observation_window_minutes: u32,
        required_checks: &[&str],
    ) -> PathBuf {
        let manifest_path = fixture.root.join("imports").join(name);
        let required = required_checks
            .iter()
            .map(|check| format!("\"{check}\""))
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
sample_routes = ["/", "/events"]

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
            .join("../../apps/harbor-shop/platform.toml")
            .canonicalize()
            .expect("sample customer app config exists")
    }

    #[test]
    fn run_from_args_returns_usage_for_help() {
        let rendered = run_from_args(["--help".to_string()]).unwrap();
        assert!(rendered.contains("platform config validate [--config <path>]"));
        assert!(rendered.contains("platform auth check [--config <path>]"));
        assert!(rendered.contains("platform auth explain [--config <path>]"));
        assert!(rendered.contains("platform module list [--config <path>]"));
        assert!(rendered.contains("platform migrate plan [--config <path>]"));
        assert!(rendered.contains("platform migrate apply [--config <path>] [--dry-run] [--yes]"));
        assert!(rendered.contains("platform release doctor [--config <path>]"));
        assert!(
            rendered
                .contains("platform cache warm [--config <path>] --scope public --route <path>")
        );
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
    fn run_from_args_uses_the_live_backend_when_deployment_enables_auth_explain() {
        let config_path = PathBuf::from("/tmp/davenda-cli-enabled.toml");
        let enabled_config = DISABLED_EXPLAIN_CONFIG
            .replace("explain_api = false", "explain_api = true")
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"platform-extended-auth\"",
            );
        fs::write(&config_path, enabled_config).unwrap();

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
        assert!(rendered.contains("storage.verify"));
        assert!(rendered.contains("legacy writes must be frozen"));
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
    fn run_from_args_observes_a_prepared_cutover_until_it_passes() {
        let fixture = import_fixture();
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
        let cutover_manifest =
            write_cutover_observe_manifest(&fixture, "cutover-observe-pass.toml", 0);

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([("/".to_string(), 200_u16), ("/events".to_string(), 200_u16)]),
        );
        let switched = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();
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
        let cutover_manifest =
            write_cutover_observe_manifest(&fixture, "cutover-observe-fail.toml", 0);

        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let probe_server = LiveProbeTestServer::spawn(
            "healthy",
            "healthy",
            false,
            BTreeMap::from([("/".to_string(), 200_u16), ("/events".to_string(), 500_u16)]),
        );
        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();
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
                    LiveProbeResponse::html(
                        200,
                        r#"<html><head><link rel="canonical" href="/" /></head><body><img src="/media/home.jpg" /></body></html>"#,
                    ),
                ),
                (
                    "/events".to_string(),
                    LiveProbeResponse::html(
                        200,
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
        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

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
                    LiveProbeResponse::html(
                        200,
                        r#"<html><head><link rel="canonical" href="/" /></head><body><img src="/media/home.jpg" /></body></html>"#,
                    ),
                ),
                (
                    "/events".to_string(),
                    LiveProbeResponse::html(
                        200,
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
        run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            probe_server.base_url().to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

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
    fn run_from_args_records_cutover_rollbacks_after_the_live_switch() {
        let fixture = import_fixture();
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
        let switched = run_from_args([
            "import".to_string(),
            "cutover".to_string(),
            cutover_manifest.display().to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();
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
    fn user_import_updates_map_administrators_into_group_and_site_admin_tuples() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["administrator"]
            }
        });

        let (updates, persisted) = user_import_updates(&staged, Some("main")).unwrap();

        assert_eq!(updates.len(), 2);
        assert_eq!(persisted["table"], "auth_tuples");
        assert_eq!(persisted["principal_id"], "alice");
        assert_eq!(persisted["site_id"], "main");
        assert_eq!(persisted["writes"], 2);
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
    fn user_import_updates_map_editors_into_group_and_site_editor_tuples() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["editor"]
            }
        });

        let (updates, persisted) = user_import_updates(&staged, Some("main")).unwrap();

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
    fn user_import_updates_reject_unsupported_legacy_roles() {
        let staged = serde_json::json!({
            "normalized": {
                "principal_id": "alice",
                "legacy_roles": ["shop_manager"]
            }
        });

        let error = user_import_updates(&staged, Some("main")).unwrap_err();
        assert!(error.to_string().contains("cannot be mapped safely"));
    }

    #[test]
    fn user_import_updates_reject_missing_required_fields() {
        let staged = serde_json::json!({
            "normalized": {
                "legacy_roles": ["administrator"]
            }
        });

        let error = user_import_updates(&staged, Some("main")).unwrap_err();
        assert!(error.to_string().contains("normalized.principal_id"));
    }
}
