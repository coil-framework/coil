use crate::CliModelError;
use crate::cli::customer_app::{load_customer_app_context, load_official_modules};
use crate::cli::args::{
    AssetsPublishInvocation, CliInput, DevServerInvocation, MigrateApplyInvocation, parse,
};
use crate::cli::auth::AuthExplainResult;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::error::CliRunError;
use crate::cli::import::ImportCutoverInvocation;
use crate::cli::render::{render_auth_explain, render_command_report};
use crate::registry::CliRuntime;
use crate::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};
use davenda_app::{CustomerAppManifest, CustomerAppRuntimePlan};
use davenda_assets::{AssetDeliveryTarget, ContentFingerprint, FingerprintAlgorithm, RevisionId};
use davenda_auth::configured_auth_model_package;
use davenda_config::{PlatformConfig, StorageClass};
use davenda_data::{
    DataValue, MigrationPlan, MigrationRegistry, MutationAction, MutationSpec, PostgresDataClient,
};
use davenda_import::{
    CutoverCheck, CutoverPlan, ImportManifest, ImportModelError, PublicationMode,
    RollbackTrigger,
};
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use davenda_runtime::{EnvironmentSecretResolver, RuntimeBuilder, StorageHost};
use davenda_storage::{StorageDeliveryLocation, StoragePlanRequest, StoragePolicy, StoragePolicyOverride};
use serde_json::Value;

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
        CliInput::ModuleList {
            output_mode,
            config_path,
        } => {
            let context = load_customer_app_context(&config_path)?;
            let auth_package =
                configured_auth_model_package(context.config.auth.package.clone());
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
            let auth_package =
                configured_auth_model_package(context.config.auth.package.clone());
            let migration_summary =
                context
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
            let auth_package =
                configured_auth_model_package(context.config.auth.package.clone());
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
            let manifest =
                ImportManifest::from_file(&invocation.manifest_path).map_err(|error| {
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

            let report = if dry_run {
                plan.command_report().map_err(|error| {
                    CliRunError::execution(format!(
                        "failed to render import plan `{}`: {error}",
                        invocation.manifest_path.display()
                    ))
                })?
            } else {
                let journal_path = import_journal_path(&invocation.manifest_path, &manifest.run_id);
                let execution = if let Some(runtime) = import_runtime.as_ref() {
                    let storage_host = runtime.storage_host.clone();
                    let default_locale = runtime.built.manifest.default_locale.to_string();
                    let publish_validated =
                        plan.publication_mode == PublicationMode::PublishValidated;
                    let data_runtime = runtime.built.runtime_plan.runtime.data.clone();
                    let mut data_client = None;
                    let tokio_runtime = if publish_validated {
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
                }
                report
            };
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
        "  platform auth explain [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]",
        "  platform module list [--config <path>] [--json]",
        "  platform migrate plan [--config <path>] [--json]",
        "  platform migrate apply [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform release doctor [--config <path>] [--json]",
        "  platform storage verify [--config <path>] [--policy] [--json]",
        "  platform assets publish [--config <path>] [--dry-run] [--yes] [--json]",
        "  platform import run <manifest-path> [--dry-run] [--json]",
        "  platform import cutover <manifest-path> [--json]",
        "",
        "Examples:",
        "  platform dev server --config config/platform.toml",
        "  platform config validate --config config/platform.toml",
        "  platform auth explain --subject user:alice --capability cms.page.publish --resource page:homepage",
        "  platform module list --config config/platform.toml",
        "  platform migrate plan --config config/platform.toml",
        "  platform migrate apply --config config/platform.toml --dry-run",
        "  platform release doctor --config config/platform.toml",
        "  platform storage verify --config config/platform.toml --policy",
        "  platform assets publish --config apps/harbor-shop/platform.toml --dry-run",
        "  platform import run imports/wordpress-events.toml",
        "  platform import run imports/wordpress-events.toml --dry-run",
        "  platform import cutover imports/wordpress-events.toml",
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

fn run_import_cutover(invocation: &ImportCutoverInvocation) -> Result<CommandReport, CliRunError> {
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
        )?);

    if let Some(verification) = &manifest.verification {
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "verification.plan",
            format!(
                "verification checks declared: {}",
                verification.required.join(", ")
            ),
            true,
            !verification.required.is_empty(),
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
        "release doctor must be fully green before traffic moves",
        true,
        release_report.status == ReportStatus::Ok,
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
        cutover_plan = cutover_plan.with_check(build_cutover_check(
            "cache.warm",
            "cache warm command surface is not implemented yet for production cutover",
            true,
            false,
        )?);
    }

    for trigger in &cutover.rollback_triggers {
        cutover_plan = cutover_plan.with_trigger(
            RollbackTrigger::new(trigger.id.clone(), trigger.description.clone())
                .map_err(|error| CliRunError::execution(error.to_string()))?,
        );
    }

    let mut report = cutover_plan
        .command_report()
        .map_err(|error| CliRunError::execution(format!("failed to render cutover plan: {error}")))?;
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

    Ok(report)
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
            applied_keys.is_none_or(|keys| !keys.contains(&(step.owner.to_string(), step.id.to_string())))
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
                applied_keys.is_none_or(|keys| !keys.contains(&(step.owner.to_string(), step.id.to_string())))
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
                .with_cell("delivery", format_asset_delivery_target(published.delivery().target()))
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

    let resolved_manifest = CustomerAppManifest::from_file(manifest_root.join(&target.app_manifest))
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

    Ok(Some(BuiltImportRuntimeContext { built, storage_host }))
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
            RevisionId::new(format!("rev-{target_id}")).map_err(|error| ImportModelError::ManifestParse {
                message: error.to_string(),
            })?,
            logical_path,
            override_policy,
            content_type,
            bytes.len() as u64,
            ContentFingerprint::new(FingerprintAlgorithm::Sha256, checksum.clone()).map_err(|error| {
                ImportModelError::ManifestParse {
                    message: error.to_string(),
                }
            })?,
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

    let connected = data_runtime
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
            let handle = thread::spawn(move || loop {
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
    "email": "alice@example.com",
    "username": "alice"
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
        fs::write(
            app_root.join("app.toml"),
            CUSTOMER_APP_MANIFEST_WITH_ASSETS,
        )
        .unwrap();
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
        assert!(rendered.contains("platform auth explain [--config <path>]"));
        assert!(rendered.contains("platform module list [--config <path>]"));
        assert!(rendered.contains("platform migrate plan [--config <path>]"));
        assert!(rendered.contains("platform migrate apply [--config <path>] [--dry-run] [--yes]"));
        assert!(rendered.contains("platform release doctor [--config <path>]"));
        assert!(rendered.contains("platform storage verify [--config <path>] [--policy]"));
        assert!(rendered.contains("platform assets publish [--config <path>] [--dry-run] [--yes]"));
        assert!(rendered.contains("platform import run <manifest-path> [--dry-run]"));
        assert!(rendered.contains("platform import cutover <manifest-path>"));
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
            error.to_string().contains("import manifest locale `fr` is not supported"),
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
}
