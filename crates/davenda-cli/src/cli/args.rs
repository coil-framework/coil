use crate::cli::config::ConfigValidateInvocation;
use crate::cli::error::CliRunError;
use crate::cli::import::{ImportCutoverInvocation, ImportRunInvocation};
use crate::command::OutputMode;
use davenda_auth::{Capability, DefaultSubject, Entity, ExplainOptions, Namespace, Relation};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplainInvocation {
    pub config_path: PathBuf,
    pub subject: DefaultSubject,
    pub capability: Capability,
    pub resource: Entity,
    pub options: ExplainOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCheckInvocation {
    pub config_path: PathBuf,
    pub subject: DefaultSubject,
    pub capability: Capability,
    pub resource: Entity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthBindingsInspectInvocation {
    pub config_path: PathBuf,
    pub capability: Option<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthTestModelInvocation {
    pub config_path: PathBuf,
    pub spec_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthListInvocation {
    pub config_path: PathBuf,
    pub subject: DefaultSubject,
    pub relation: Relation,
    pub namespace: Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthLookupInvocation {
    pub config_path: PathBuf,
    pub resource: Entity,
    pub relation: Relation,
    pub subject_namespace: Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPackageValidateInvocation {
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPackageInspectInvocation {
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleInspectInvocation {
    pub config_path: PathBuf,
    pub module: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleInstallInvocation {
    pub config_path: PathBuf,
    pub module: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleEnableInvocation {
    pub config_path: PathBuf,
    pub module: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ModuleDisableInvocation {
    pub config_path: PathBuf,
    pub module: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DevServerInvocation {
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrateApplyInvocation {
    pub config_path: PathBuf,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssetsPublishInvocation {
    pub config_path: PathBuf,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheWarmInvocation {
    pub config_path: PathBuf,
    pub scope: String,
    pub routes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheInspectInvocation {
    pub config_path: PathBuf,
    pub route: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CacheInvalidateInvocation {
    pub config_path: PathBuf,
    pub tags: Vec<String>,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsStatusInvocation {
    pub config_path: PathBuf,
    pub queue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsRunInvocation {
    pub config_path: PathBuf,
    pub queue: Option<String>,
    pub worker_id: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsReadyInvocation {
    pub config_path: PathBuf,
    pub queue: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsDeadLettersInvocation {
    pub config_path: PathBuf,
    pub queue: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsInFlightInvocation {
    pub config_path: PathBuf,
    pub queue: Option<String>,
    pub worker_id: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsRetryInvocation {
    pub config_path: PathBuf,
    pub dead_letter_id: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobsPromoteInvocation {
    pub config_path: PathBuf,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsRenewInvocation {
    pub config_path: PathBuf,
    pub certificate_id: String,
    pub replacement_certificate_id: String,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageInspectInvocation {
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliInput {
    Help,
    DevServer {
        invocation: DevServerInvocation,
    },
    ConfigValidate {
        output_mode: OutputMode,
        invocation: ConfigValidateInvocation,
    },
    AuthExplain {
        output_mode: OutputMode,
        invocation: AuthExplainInvocation,
    },
    AuthCheck {
        output_mode: OutputMode,
        invocation: AuthCheckInvocation,
    },
    AuthBindingsInspect {
        output_mode: OutputMode,
        invocation: AuthBindingsInspectInvocation,
    },
    AuthTestModel {
        output_mode: OutputMode,
        invocation: AuthTestModelInvocation,
    },
    AuthList {
        output_mode: OutputMode,
        invocation: AuthListInvocation,
    },
    AuthLookup {
        output_mode: OutputMode,
        invocation: AuthLookupInvocation,
    },
    AuthPackageValidate {
        output_mode: OutputMode,
        invocation: AuthPackageValidateInvocation,
    },
    AuthPackageInspect {
        output_mode: OutputMode,
        invocation: AuthPackageInspectInvocation,
    },
    ModuleList {
        output_mode: OutputMode,
        config_path: PathBuf,
    },
    ModuleInspect {
        output_mode: OutputMode,
        invocation: ModuleInspectInvocation,
    },
    ModuleInstall {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: ModuleInstallInvocation,
    },
    ModuleEnable {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: ModuleEnableInvocation,
    },
    ModuleDisable {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: ModuleDisableInvocation,
    },
    MigratePlan {
        output_mode: OutputMode,
        config_path: PathBuf,
    },
    MigrateApply {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: MigrateApplyInvocation,
    },
    ReleaseDoctor {
        output_mode: OutputMode,
        config_path: PathBuf,
    },
    ReleasePlan {
        output_mode: OutputMode,
        config_path: PathBuf,
    },
    CacheWarm {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: CacheWarmInvocation,
    },
    CacheInspect {
        output_mode: OutputMode,
        invocation: CacheInspectInvocation,
    },
    CacheInvalidate {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: CacheInvalidateInvocation,
    },
    JobsStatus {
        output_mode: OutputMode,
        invocation: JobsStatusInvocation,
    },
    JobsRun {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: JobsRunInvocation,
    },
    JobsReady {
        output_mode: OutputMode,
        invocation: JobsReadyInvocation,
    },
    JobsDeadLetters {
        output_mode: OutputMode,
        invocation: JobsDeadLettersInvocation,
    },
    JobsInFlight {
        output_mode: OutputMode,
        invocation: JobsInFlightInvocation,
    },
    JobsRetry {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: JobsRetryInvocation,
    },
    JobsPromote {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: JobsPromoteInvocation,
    },
    TlsStatus {
        output_mode: OutputMode,
        config_path: PathBuf,
    },
    TlsRenew {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: TlsRenewInvocation,
    },
    StorageInspect {
        output_mode: OutputMode,
        invocation: StorageInspectInvocation,
    },
    StorageVerify {
        output_mode: OutputMode,
        config_path: PathBuf,
        verify_policy: bool,
    },
    AssetsPublish {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: AssetsPublishInvocation,
    },
    ImportRun {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: ImportRunInvocation,
    },
    ImportCutover {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: ImportCutoverInvocation,
    },
}

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<CliInput, CliRunError> {
    let mut config_path = std::env::var("DAVENDA_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let mut output_mode = OutputMode::Human;
    let mut dry_run = false;
    let mut confirmed = false;
    let mut apply_cutover = false;
    let mut switch_cutover = false;
    let mut observe_cutover = false;
    let mut rollback_cutover = false;
    let mut cutover_base_url: Option<String> = None;
    let mut cutover_switch_plan_path: Option<PathBuf> = None;
    let mut cutover_switch_zone_id: Option<String> = None;
    let mut cutover_switch_resource_id: Option<String> = None;
    let mut cutover_switch_target: Option<String> = None;
    let mut cutover_dns_zone_id: Option<String> = None;
    let mut cutover_dns_target: Option<String> = None;
    let mut cutover_reason: Option<String> = None;
    let mut legacy_freeze_confirmed = false;
    let mut verify_policy = false;
    let mut cache_scope: Option<String> = None;
    let mut cache_routes = Vec::new();
    let mut cache_tags = Vec::new();
    let mut jobs_queue: Option<String> = None;
    let mut jobs_worker_id: Option<String> = None;
    let mut jobs_limit: Option<usize> = None;
    let mut tls_certificate_id: Option<String> = None;
    let mut tls_replacement_certificate_id: Option<String> = None;
    let mut subject: Option<DefaultSubject> = None;
    let mut relation: Option<Relation> = None;
    let mut namespace: Option<Namespace> = None;
    let mut subject_namespace: Option<Namespace> = None;
    let mut capability: Option<Capability> = None;
    let mut resource: Option<Entity> = None;
    let mut max_depth: Option<usize> = None;
    let mut cycle_protection = true;
    let mut positionals = Vec::new();

    let mut iter = args.into_iter();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--help" | "-h" => return Ok(CliInput::Help),
            "--json" => output_mode = OutputMode::Json,
            "--dry-run" => dry_run = true,
            "--yes" => confirmed = true,
            "--apply" => apply_cutover = true,
            "--switch" => switch_cutover = true,
            "--observe" => observe_cutover = true,
            "--rollback" => rollback_cutover = true,
            "--base-url" => {
                cutover_base_url = Some(next_value(&mut iter, "--base-url")?);
            }
            "--switch-plan" => {
                cutover_switch_plan_path =
                    Some(PathBuf::from(next_value(&mut iter, "--switch-plan")?));
            }
            "--switch-zone-id" => {
                cutover_switch_zone_id = Some(next_value(&mut iter, "--switch-zone-id")?);
            }
            "--switch-resource-id" => {
                cutover_switch_resource_id = Some(next_value(&mut iter, "--switch-resource-id")?);
            }
            "--switch-target" => {
                cutover_switch_target = Some(next_value(&mut iter, "--switch-target")?);
            }
            "--dns-zone-id" => {
                cutover_dns_zone_id = Some(next_value(&mut iter, "--dns-zone-id")?);
            }
            "--dns-target" => {
                cutover_dns_target = Some(next_value(&mut iter, "--dns-target")?);
            }
            "--reason" => {
                cutover_reason = Some(next_value(&mut iter, "--reason")?);
            }
            "--legacy-freeze-confirmed" => legacy_freeze_confirmed = true,
            "--policy" => verify_policy = true,
            "--scope" => {
                cache_scope = Some(next_value(&mut iter, "--scope")?);
            }
            "--route" => {
                cache_routes.push(next_value(&mut iter, "--route")?);
            }
            "--tag" => {
                cache_tags.push(next_value(&mut iter, "--tag")?);
            }
            "--queue" => {
                jobs_queue = Some(next_value(&mut iter, "--queue")?);
            }
            "--worker-id" => {
                jobs_worker_id = Some(next_value(&mut iter, "--worker-id")?);
            }
            "--limit" => {
                let parsed = next_value(&mut iter, "--limit")?
                    .parse::<usize>()
                    .map_err(|_| {
                        CliRunError::usage("`--limit` must be a valid positive integer")
                    })?;
                if parsed == 0 {
                    return Err(CliRunError::usage("`--limit` must be greater than zero"));
                }
                jobs_limit = Some(parsed);
            }
            "--certificate" => {
                tls_certificate_id = Some(next_value(&mut iter, "--certificate")?);
            }
            "--replacement" => {
                tls_replacement_certificate_id = Some(next_value(&mut iter, "--replacement")?);
            }
            "--config" => {
                config_path = Some(PathBuf::from(next_value(&mut iter, "--config")?));
            }
            "--subject" => {
                subject = Some(parse_subject(&next_value(&mut iter, "--subject")?)?);
            }
            "--relation" => {
                relation = Some(parse_relation(&next_value(&mut iter, "--relation")?)?);
            }
            "--namespace" => {
                namespace = Some(parse_namespace(&next_value(&mut iter, "--namespace")?)?);
            }
            "--subject-namespace" => {
                subject_namespace = Some(parse_namespace(&next_value(
                    &mut iter,
                    "--subject-namespace",
                )?)?);
            }
            "--capability" => {
                capability = Some(parse_capability(&next_value(&mut iter, "--capability")?)?);
            }
            "--resource" => {
                resource = Some(parse_entity(&next_value(&mut iter, "--resource")?)?);
            }
            "--object" => {
                resource = Some(parse_entity(&next_value(&mut iter, "--object")?)?);
            }
            "--max-depth" => {
                max_depth = Some(
                    next_value(&mut iter, "--max-depth")?
                        .parse::<usize>()
                        .map_err(|_| CliRunError::usage("`--max-depth` must be a valid integer"))?,
                );
            }
            "--no-cycle-protection" => {
                cycle_protection = false;
            }
            flag if flag.starts_with('-') => {
                return Err(CliRunError::usage(format!("unknown CLI flag `{flag}`")));
            }
            positional => positionals.push(positional.to_string()),
        }
    }

    if positionals.is_empty() {
        return Ok(CliInput::Help);
    }

    match positionals.as_slice() {
        [command, subcommand] if command == "dev" && subcommand == "server" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`dev server` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::DevServer {
                invocation: DevServerInvocation { config_path },
            })
        }
        [command, subcommand] if command == "config" && subcommand == "validate" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`config validate` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ConfigValidate {
                output_mode,
                invocation: ConfigValidateInvocation { config_path },
            })
        }
        [command, subcommand] if command == "auth" && subcommand == "explain" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth explain` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let subject = subject.ok_or_else(|| {
                CliRunError::usage("`auth explain` requires `--subject <subject>`")
            })?;
            let capability = capability.ok_or_else(|| {
                CliRunError::usage("`auth explain` requires `--capability <capability>`")
            })?;
            let resource = resource.ok_or_else(|| {
                CliRunError::usage("`auth explain` requires `--resource <namespace:id>`")
            })?;

            let options = match max_depth {
                Some(depth) => ExplainOptions::new(depth),
                None => ExplainOptions::default(),
            }
            .with_cycle_protection(cycle_protection)
            .normalized();

            Ok(CliInput::AuthExplain {
                output_mode,
                invocation: AuthExplainInvocation {
                    config_path,
                    subject,
                    capability,
                    resource,
                    options,
                },
            })
        }
        [command, subcommand] if command == "auth" && subcommand == "check" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth check` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let subject = subject
                .ok_or_else(|| CliRunError::usage("`auth check` requires `--subject <subject>`"))?;
            let capability = capability.ok_or_else(|| {
                CliRunError::usage("`auth check` requires `--capability <capability>`")
            })?;
            let resource = resource.ok_or_else(|| {
                CliRunError::usage("`auth check` requires `--resource <namespace:id>`")
            })?;

            Ok(CliInput::AuthCheck {
                output_mode,
                invocation: AuthCheckInvocation {
                    config_path,
                    subject,
                    capability,
                    resource,
                },
            })
        }
        [command, group, subcommand]
            if command == "auth" && group == "bindings" && subcommand == "inspect" =>
        {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth bindings inspect` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::AuthBindingsInspect {
                output_mode,
                invocation: AuthBindingsInspectInvocation {
                    config_path,
                    capability,
                },
            })
        }
        [command, subcommand, spec_path] if command == "auth" && subcommand == "test-model" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth test-model` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::AuthTestModel {
                output_mode,
                invocation: AuthTestModelInvocation {
                    config_path,
                    spec_path: PathBuf::from(spec_path),
                },
            })
        }
        [command, subcommand] if command == "auth" && subcommand == "list" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth list` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let subject = subject
                .ok_or_else(|| CliRunError::usage("`auth list` requires `--subject <subject>`"))?;
            let relation = relation.ok_or_else(|| {
                CliRunError::usage("`auth list` requires `--relation <relation>`")
            })?;
            let namespace = namespace.ok_or_else(|| {
                CliRunError::usage("`auth list` requires `--namespace <namespace>`")
            })?;

            Ok(CliInput::AuthList {
                output_mode,
                invocation: AuthListInvocation {
                    config_path,
                    subject,
                    relation,
                    namespace,
                },
            })
        }
        [command, subcommand] if command == "auth" && subcommand == "lookup" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth lookup` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let resource = resource.ok_or_else(|| {
                CliRunError::usage("`auth lookup` requires `--resource <namespace:id>`")
            })?;
            let relation = relation.ok_or_else(|| {
                CliRunError::usage("`auth lookup` requires `--relation <relation>`")
            })?;
            let subject_namespace = subject_namespace.ok_or_else(|| {
                CliRunError::usage("`auth lookup` requires `--subject-namespace <namespace>`")
            })?;

            Ok(CliInput::AuthLookup {
                output_mode,
                invocation: AuthLookupInvocation {
                    config_path,
                    resource,
                    relation,
                    subject_namespace,
                },
            })
        }
        [command, group, subcommand]
            if command == "auth" && group == "package" && subcommand == "validate" =>
        {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth package validate` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::AuthPackageValidate {
                output_mode,
                invocation: AuthPackageValidateInvocation { config_path },
            })
        }
        [command, group, subcommand]
            if command == "auth" && group == "package" && subcommand == "inspect" =>
        {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`auth package inspect` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::AuthPackageInspect {
                output_mode,
                invocation: AuthPackageInspectInvocation { config_path },
            })
        }
        [command, subcommand] if command == "module" && subcommand == "list" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`module list` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ModuleList {
                output_mode,
                config_path,
            })
        }
        [command, subcommand, module] if command == "module" && subcommand == "inspect" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`module inspect` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ModuleInspect {
                output_mode,
                invocation: ModuleInspectInvocation {
                    config_path,
                    module: module.to_string(),
                },
            })
        }
        [command, subcommand, module] if command == "module" && subcommand == "install" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`module install` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ModuleInstall {
                output_mode,
                dry_run,
                invocation: ModuleInstallInvocation {
                    config_path,
                    module: module.to_string(),
                    confirmed,
                },
            })
        }
        [command, subcommand, module] if command == "module" && subcommand == "enable" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`module enable` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ModuleEnable {
                output_mode,
                dry_run,
                invocation: ModuleEnableInvocation {
                    config_path,
                    module: module.to_string(),
                    confirmed,
                },
            })
        }
        [command, subcommand, module] if command == "module" && subcommand == "disable" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`module disable` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ModuleDisable {
                output_mode,
                dry_run,
                invocation: ModuleDisableInvocation {
                    config_path,
                    module: module.to_string(),
                    confirmed,
                },
            })
        }
        [command, subcommand] if command == "migrate" && subcommand == "plan" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`migrate plan` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::MigratePlan {
                output_mode,
                config_path,
            })
        }
        [command, subcommand] if command == "migrate" && subcommand == "apply" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`migrate apply` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::MigrateApply {
                output_mode,
                dry_run,
                invocation: MigrateApplyInvocation {
                    config_path,
                    confirmed,
                },
            })
        }
        [command, subcommand] if command == "release" && subcommand == "doctor" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`release doctor` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ReleaseDoctor {
                output_mode,
                config_path,
            })
        }
        [command, subcommand] if command == "release" && subcommand == "plan" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`release plan` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::ReleasePlan {
                output_mode,
                config_path,
            })
        }
        [command, subcommand] if command == "cache" && subcommand == "warm" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`cache warm` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let scope = cache_scope.unwrap_or_else(|| "public".to_string());
            if scope != "public" {
                return Err(CliRunError::usage(
                    "`cache warm` currently supports only `--scope public`",
                ));
            }
            if cache_routes.is_empty() {
                return Err(CliRunError::usage(
                    "`cache warm` requires at least one `--route <path>`",
                ));
            }

            Ok(CliInput::CacheWarm {
                output_mode,
                dry_run,
                invocation: CacheWarmInvocation {
                    config_path,
                    scope,
                    routes: cache_routes,
                },
            })
        }
        [command, subcommand] if command == "cache" && subcommand == "inspect" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`cache inspect` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let [route] = cache_routes.as_slice() else {
                return Err(CliRunError::usage(
                    "`cache inspect` requires exactly one `--route <path>`",
                ));
            };

            Ok(CliInput::CacheInspect {
                output_mode,
                invocation: CacheInspectInvocation {
                    config_path,
                    route: route.clone(),
                },
            })
        }
        [command, subcommand] if command == "cache" && subcommand == "invalidate" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`cache invalidate` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            if cache_tags.is_empty() {
                return Err(CliRunError::usage(
                    "`cache invalidate` requires at least one `--tag <tag>`",
                ));
            }

            Ok(CliInput::CacheInvalidate {
                output_mode,
                dry_run,
                invocation: CacheInvalidateInvocation {
                    config_path,
                    tags: cache_tags,
                    confirmed,
                },
            })
        }
        [command, subcommand] if command == "jobs" && subcommand == "status" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs status` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsStatus {
                output_mode,
                invocation: JobsStatusInvocation {
                    config_path,
                    queue: jobs_queue,
                },
            })
        }
        [command, subcommand] if command == "jobs" && subcommand == "run" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs run` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsRun {
                output_mode,
                dry_run,
                invocation: JobsRunInvocation {
                    config_path,
                    queue: jobs_queue,
                    worker_id: jobs_worker_id,
                    limit: jobs_limit.unwrap_or(50),
                },
            })
        }
        [command, subcommand] if command == "jobs" && subcommand == "ready" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs ready` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsReady {
                output_mode,
                invocation: JobsReadyInvocation {
                    config_path,
                    queue: jobs_queue,
                    limit: jobs_limit.unwrap_or(50),
                },
            })
        }
        [command, subcommand] if command == "jobs" && subcommand == "dead-letters" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs dead-letters` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsDeadLetters {
                output_mode,
                invocation: JobsDeadLettersInvocation {
                    config_path,
                    queue: jobs_queue,
                    limit: jobs_limit.unwrap_or(50),
                },
            })
        }
        [command, subcommand] if command == "jobs" && subcommand == "in-flight" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs in-flight` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsInFlight {
                output_mode,
                invocation: JobsInFlightInvocation {
                    config_path,
                    queue: jobs_queue,
                    worker_id: jobs_worker_id,
                    limit: jobs_limit.unwrap_or(50),
                },
            })
        }
        [command, subcommand, dead_letter_id] if command == "jobs" && subcommand == "retry" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs retry` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsRetry {
                output_mode,
                dry_run,
                invocation: JobsRetryInvocation {
                    config_path,
                    dead_letter_id: dead_letter_id.to_string(),
                    confirmed,
                },
            })
        }
        [command, subcommand] if command == "jobs" && subcommand == "promote" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`jobs promote` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::JobsPromote {
                output_mode,
                dry_run,
                invocation: JobsPromoteInvocation {
                    config_path,
                    confirmed,
                },
            })
        }
        [command, subcommand] if command == "tls" && subcommand == "status" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`tls status` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::TlsStatus {
                output_mode,
                config_path,
            })
        }
        [command, subcommand] if command == "tls" && subcommand == "renew" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`tls renew` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;
            let certificate_id = tls_certificate_id
                .ok_or_else(|| CliRunError::usage("`tls renew` requires `--certificate <id>`"))?;
            let replacement_certificate_id = tls_replacement_certificate_id
                .ok_or_else(|| CliRunError::usage("`tls renew` requires `--replacement <id>`"))?;

            Ok(CliInput::TlsRenew {
                output_mode,
                dry_run,
                invocation: TlsRenewInvocation {
                    config_path,
                    certificate_id,
                    replacement_certificate_id,
                    confirmed,
                },
            })
        }
        [command, subcommand] if command == "storage" && subcommand == "verify" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`storage verify` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::StorageVerify {
                output_mode,
                config_path,
                verify_policy,
            })
        }
        [command, subcommand] if command == "storage" && subcommand == "inspect" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`storage inspect` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::StorageInspect {
                output_mode,
                invocation: StorageInspectInvocation { config_path },
            })
        }
        [command, subcommand] if command == "assets" && subcommand == "publish" => {
            let config_path = config_path
                .or_else(discover_default_config_path)
                .ok_or_else(|| {
                    CliRunError::usage(
                        "`assets publish` requires `--config <path>`, `DAVENDA_CONFIG`, or a default config file",
                    )
                })?;

            Ok(CliInput::AssetsPublish {
                output_mode,
                dry_run,
                invocation: AssetsPublishInvocation {
                    config_path,
                    confirmed,
                },
            })
        }
        [command, subcommand, manifest_path] if command == "import" && subcommand == "run" => {
            Ok(CliInput::ImportRun {
                output_mode,
                dry_run,
                invocation: ImportRunInvocation {
                    manifest_path: PathBuf::from(manifest_path),
                },
            })
        }
        [command, subcommand, manifest_path] if command == "import" && subcommand == "cutover" => {
            Ok(CliInput::ImportCutover {
                output_mode,
                dry_run,
                invocation: ImportCutoverInvocation {
                    manifest_path: PathBuf::from(manifest_path),
                    dry_run,
                    apply: apply_cutover,
                    switch: switch_cutover,
                    observe: observe_cutover,
                    rollback: rollback_cutover,
                    base_url: cutover_base_url,
                    switch_plan_path: cutover_switch_plan_path,
                    switch_zone_id: cutover_switch_zone_id,
                    switch_resource_id: cutover_switch_resource_id,
                    switch_target: cutover_switch_target,
                    dns_zone_id: cutover_dns_zone_id,
                    dns_target: cutover_dns_target,
                    reason: cutover_reason,
                    confirmed,
                    legacy_freeze_confirmed,
                },
            })
        }
        [command, subcommand] => Err(CliRunError::usage(format!(
            "unsupported command `{command} {subcommand}`"
        ))),
        [command, rest @ ..] => Err(CliRunError::usage(format!(
            "unsupported command path `{}`",
            std::iter::once(command)
                .chain(rest.iter())
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        ))),
        [] => Ok(CliInput::Help),
    }
}

fn next_value(iter: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, CliRunError> {
    iter.next()
        .ok_or_else(|| CliRunError::usage(format!("`{flag}` expects a value")))
}

fn discover_default_config_path() -> Option<PathBuf> {
    [
        PathBuf::from("davenda.toml"),
        PathBuf::from("config/davenda.toml"),
        PathBuf::from("platform.toml"),
        PathBuf::from("config/platform.toml"),
        PathBuf::from("apps/harbor-shop/platform.toml"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub(crate) fn parse_subject(input: &str) -> Result<DefaultSubject, CliRunError> {
    let (left, relation) = match input.split_once('#') {
        Some((left, relation)) => (left, Some(relation)),
        None => (input, None),
    };
    let entity = parse_entity(left)?;

    match relation {
        Some(relation) => {
            let relation = parse_relation(relation)?;
            Ok(DefaultSubject::userset(entity, relation))
        }
        None => Ok(DefaultSubject::entity(entity)),
    }
}

pub(crate) fn parse_entity(input: &str) -> Result<Entity, CliRunError> {
    let (namespace, id) = input.split_once(':').ok_or_else(|| {
        CliRunError::usage(format!("invalid entity `{input}`; expected namespace:id"))
    })?;
    if id.trim().is_empty() {
        return Err(CliRunError::usage(format!(
            "invalid entity `{input}`; the identifier cannot be empty"
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
            return Err(CliRunError::usage(format!(
                "unknown entity namespace `{other}`"
            )));
        }
    };

    Ok(entity)
}

fn parse_relation(input: &str) -> Result<Relation, CliRunError> {
    Relation::from_str(input)
        .ok_or_else(|| CliRunError::usage(format!("unknown relation `{input}`")))
}

fn parse_namespace(input: &str) -> Result<Namespace, CliRunError> {
    Namespace::from_str(input)
        .ok_or_else(|| CliRunError::usage(format!("unknown namespace `{input}`")))
}

pub(crate) fn parse_capability(input: &str) -> Result<Capability, CliRunError> {
    let capability = match input {
        "system.module.manage" => Capability::SystemModuleManage,
        "system.config.read" => Capability::SystemConfigRead,
        "system.config.write" => Capability::SystemConfigWrite,
        "admin.shell.access" => Capability::AdminShellAccess,
        "admin.audit.read" => Capability::AdminAuditRead,
        "cms.page.read" => Capability::CmsPageRead,
        "cms.page.publish" => Capability::CmsPagePublish,
        "cms.page.edit" => Capability::CmsPageEdit,
        "cms.navigation.edit" => Capability::CmsNavigationEdit,
        "catalog.product.read" => Capability::CatalogProductRead,
        "catalog.product.edit" => Capability::CatalogProductEdit,
        "catalog.collection.edit" => Capability::CatalogCollectionEdit,
        "checkout.session.create" => Capability::CheckoutSessionCreate,
        "order.read" => Capability::OrderRead,
        "order.refund.issue" => Capability::OrderRefundIssue,
        "membership.subscription.manage" => Capability::MembershipSubscriptionManage,
        "membership.tier.edit" => Capability::MembershipTierEdit,
        "events.event.publish" => Capability::EventsEventPublish,
        "events.slot.manage" => Capability::EventsSlotManage,
        "events.booking.create" => Capability::EventsBookingCreate,
        "events.booking.check_in" => Capability::EventsBookingCheckIn,
        "asset.read" => Capability::AssetRead,
        "asset.read_public" => Capability::AssetReadPublic,
        "asset.publish" => Capability::AssetPublish,
        "asset.replace" => Capability::AssetReplace,
        "asset.manage_storage" => Capability::AssetManageStorage,
        "seo.metadata.edit" => Capability::SeoMetadataEdit,
        "i18n.translation.edit" => Capability::I18nTranslationEdit,
        other => {
            return Err(CliRunError::usage(format!("unknown capability `{other}`")));
        }
    };

    Ok(capability)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_auth_explain_requires_config_and_live_subjects() {
        let input = parse([
            "auth".to_string(),
            "explain".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "admin.audit.read".to_string(),
            "--resource".to_string(),
            "admin_module:app".to_string(),
        ])
        .unwrap();

        let CliInput::AuthExplain { invocation, .. } = input else {
            panic!("expected auth explain input");
        };
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
        assert_eq!(invocation.resource, Entity::admin_module("app"));
        assert!(invocation.options.cycle_protection);
    }

    #[test]
    fn parse_auth_check_requires_subject_capability_and_resource() {
        let input = parse([
            "auth".to_string(),
            "check".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "admin.audit.read".to_string(),
            "--resource".to_string(),
            "admin_module:app".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthCheck {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth check input");
        };
        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
        assert_eq!(invocation.resource, Entity::admin_module("app"));
    }

    #[test]
    fn parse_auth_bindings_inspect_accepts_optional_capability_filter() {
        let input = parse([
            "auth".to_string(),
            "bindings".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--capability".to_string(),
            "cms.page.read".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthBindingsInspect {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth bindings inspect input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.capability, Some(Capability::CmsPageRead));
    }

    #[test]
    fn parse_auth_list_requires_subject_relation_and_namespace() {
        let input = parse([
            "auth".to_string(),
            "list".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--relation".to_string(),
            "view".to_string(),
            "--namespace".to_string(),
            "page".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthList {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth list input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(
            invocation.subject,
            DefaultSubject::entity(Entity::user("alice"))
        );
        assert_eq!(invocation.relation, Relation::View);
        assert_eq!(invocation.namespace, Namespace::Page);
    }

    #[test]
    fn parse_auth_test_model_uses_explicit_config_and_spec_path() {
        let input = parse([
            "auth".to_string(),
            "test-model".to_string(),
            "fixtures/auth-model.toml".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthTestModel {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth test-model input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(
            invocation.spec_path,
            PathBuf::from("fixtures/auth-model.toml")
        );
    }

    #[test]
    fn parse_auth_lookup_requires_resource_relation_and_subject_namespace() {
        let input = parse([
            "auth".to_string(),
            "lookup".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
            "--relation".to_string(),
            "view".to_string(),
            "--subject-namespace".to_string(),
            "user".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthLookup {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth lookup input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.resource, Entity::page("homepage"));
        assert_eq!(invocation.relation, Relation::View);
        assert_eq!(invocation.subject_namespace, Namespace::User);
    }

    #[test]
    fn parse_auth_package_validate_uses_explicit_config_path() {
        let input = parse([
            "auth".to_string(),
            "package".to_string(),
            "validate".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthPackageValidate {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth package validate input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
    }

    #[test]
    fn parse_auth_package_inspect_uses_explicit_config_path() {
        let input = parse([
            "auth".to_string(),
            "package".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthPackageInspect {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth package inspect input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
    }

    #[test]
    fn parse_module_inspect_uses_explicit_config_path_and_module_name() {
        let input = parse([
            "module".to_string(),
            "inspect".to_string(),
            "cms".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ModuleInspect {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected module inspect input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.module, "cms");
    }

    #[test]
    fn parse_module_install_accepts_dry_run_and_confirmation_flags() {
        let input = parse([
            "module".to_string(),
            "install".to_string(),
            "media".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--dry-run".to_string(),
            "--yes".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ModuleInstall {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected module install input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.module, "media");
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_module_enable_accepts_dry_run_and_confirmation_flags() {
        let input = parse([
            "module".to_string(),
            "enable".to_string(),
            "media".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--dry-run".to_string(),
            "--yes".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ModuleEnable {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected module enable input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.module, "media");
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_module_disable_accepts_confirmation_flag() {
        let input = parse([
            "module".to_string(),
            "disable".to_string(),
            "media".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--yes".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ModuleDisable {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected module disable input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(!dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.module, "media");
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_release_plan_uses_explicit_config_path() {
        let input = parse([
            "release".to_string(),
            "plan".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ReleasePlan {
            output_mode,
            config_path,
        } = input
        else {
            panic!("expected release plan input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(config_path, PathBuf::from("/tmp/platform.toml"));
    }

    #[test]
    fn parse_import_run_accepts_manifest_and_execution_flags() {
        let input = parse([
            "import".to_string(),
            "run".to_string(),
            "imports/wordpress-events.toml".to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ImportRun {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected import run input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(
            invocation.manifest_path,
            PathBuf::from("imports/wordpress-events.toml")
        );
    }

    #[test]
    fn parse_import_cutover_accepts_a_manifest_path() {
        let input = parse([
            "import".to_string(),
            "cutover".to_string(),
            "imports/wordpress-events.toml".to_string(),
            "--apply".to_string(),
            "--yes".to_string(),
            "--legacy-freeze-confirmed".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ImportCutover {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected import cutover input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(!dry_run);
        assert_eq!(
            invocation.manifest_path,
            PathBuf::from("imports/wordpress-events.toml")
        );
        assert!(!invocation.dry_run);
        assert!(invocation.apply);
        assert!(invocation.confirmed);
        assert!(invocation.legacy_freeze_confirmed);
        assert!(!invocation.switch);
        assert!(!invocation.observe);
        assert!(!invocation.rollback);
        assert_eq!(invocation.base_url, None);
        assert_eq!(invocation.switch_plan_path, None);
        assert_eq!(invocation.switch_zone_id, None);
        assert_eq!(invocation.switch_resource_id, None);
        assert_eq!(invocation.switch_target, None);
        assert_eq!(invocation.dns_zone_id, None);
        assert_eq!(invocation.dns_target, None);
        assert_eq!(invocation.reason, None);
    }

    #[test]
    fn parse_import_cutover_accepts_observation_inputs() {
        let input = parse([
            "import".to_string(),
            "cutover".to_string(),
            "imports/wordpress-events.toml".to_string(),
            "--observe".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let CliInput::ImportCutover {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected import cutover input");
        };

        assert_eq!(output_mode, OutputMode::Human);
        assert!(!dry_run);
        assert_eq!(
            invocation.manifest_path,
            PathBuf::from("imports/wordpress-events.toml")
        );
        assert!(!invocation.dry_run);
        assert!(!invocation.apply);
        assert!(!invocation.switch);
        assert!(invocation.observe);
        assert!(!invocation.rollback);
        assert_eq!(
            invocation.base_url.as_deref(),
            Some("https://shop.example.com")
        );
        assert_eq!(invocation.switch_plan_path, None);
        assert_eq!(invocation.switch_zone_id, None);
        assert_eq!(invocation.switch_resource_id, None);
        assert_eq!(invocation.switch_target, None);
        assert_eq!(invocation.dns_zone_id, None);
        assert_eq!(invocation.dns_target, None);
        assert_eq!(invocation.reason, None);
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_import_cutover_accepts_switch_and_rollback_inputs() {
        let input = parse([
            "import".to_string(),
            "cutover".to_string(),
            "imports/wordpress-events.toml".to_string(),
            "--switch".to_string(),
            "--dry-run".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--switch-plan".to_string(),
            "cutover/load-balancer.json".to_string(),
            "--dns-zone-id".to_string(),
            "zone-123".to_string(),
            "--dns-target".to_string(),
            "davenda-origin.example.net".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let CliInput::ImportCutover {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected import cutover input");
        };

        assert_eq!(output_mode, OutputMode::Human);
        assert!(dry_run);
        assert!(invocation.dry_run);
        assert!(!invocation.apply);
        assert!(invocation.switch);
        assert!(!invocation.observe);
        assert!(!invocation.rollback);
        assert_eq!(
            invocation.base_url.as_deref(),
            Some("https://shop.example.com")
        );
        assert_eq!(
            invocation.switch_plan_path,
            Some(PathBuf::from("cutover/load-balancer.json"))
        );
        assert_eq!(invocation.switch_zone_id, None);
        assert_eq!(invocation.switch_resource_id, None);
        assert_eq!(invocation.switch_target, None);
        assert_eq!(invocation.dns_zone_id.as_deref(), Some("zone-123"));
        assert_eq!(
            invocation.dns_target.as_deref(),
            Some("davenda-origin.example.net")
        );
        assert_eq!(invocation.reason, None);
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_import_cutover_accepts_rollback_inputs() {
        let input = parse([
            "import".to_string(),
            "cutover".to_string(),
            "imports/wordpress-events.toml".to_string(),
            "--rollback".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--reason".to_string(),
            "auth failure".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let CliInput::ImportCutover {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected import cutover input");
        };

        assert_eq!(output_mode, OutputMode::Human);
        assert!(!dry_run);
        assert!(!invocation.dry_run);
        assert!(!invocation.apply);
        assert!(!invocation.switch);
        assert!(!invocation.observe);
        assert!(invocation.rollback);
        assert_eq!(
            invocation.base_url.as_deref(),
            Some("https://shop.example.com")
        );
        assert_eq!(invocation.switch_plan_path, None);
        assert_eq!(invocation.switch_zone_id, None);
        assert_eq!(invocation.switch_resource_id, None);
        assert_eq!(invocation.switch_target, None);
        assert_eq!(invocation.dns_zone_id, None);
        assert_eq!(invocation.dns_target, None);
        assert_eq!(invocation.reason.as_deref(), Some("auth failure"));
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_import_cutover_accepts_generic_switch_inputs() {
        let input = parse([
            "import".to_string(),
            "cutover".to_string(),
            "imports/wordpress-events.toml".to_string(),
            "--switch".to_string(),
            "--base-url".to_string(),
            "https://shop.example.com".to_string(),
            "--switch-zone-id".to_string(),
            "zone-123".to_string(),
            "--switch-resource-id".to_string(),
            "lb-edge-1".to_string(),
            "--switch-target".to_string(),
            "davenda-origin-pool".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let CliInput::ImportCutover { invocation, .. } = input else {
            panic!("expected import cutover input");
        };

        assert!(invocation.switch);
        assert_eq!(invocation.switch_zone_id.as_deref(), Some("zone-123"));
        assert_eq!(invocation.switch_resource_id.as_deref(), Some("lb-edge-1"));
        assert_eq!(
            invocation.switch_target.as_deref(),
            Some("davenda-origin-pool")
        );
        assert_eq!(invocation.dns_zone_id, None);
        assert_eq!(invocation.dns_target, None);
    }

    #[test]
    fn parse_cache_warm_accepts_scope_routes_and_dry_run() {
        let input = parse([
            "cache".to_string(),
            "warm".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--scope".to_string(),
            "public".to_string(),
            "--route".to_string(),
            "/en-GB/home".to_string(),
            "--route".to_string(),
            "/en-GB/events".to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::CacheWarm {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected cache warm input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
        assert_eq!(invocation.scope, "public");
        assert_eq!(
            invocation.routes,
            vec!["/en-GB/home".to_string(), "/en-GB/events".to_string()]
        );
    }

    #[test]
    fn parse_cache_inspect_accepts_exactly_one_route() {
        let input = parse([
            "cache".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--route".to_string(),
            "/en-GB/home".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::CacheInspect {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected cache inspect input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
        assert_eq!(invocation.route, "/en-GB/home");
    }

    #[test]
    fn parse_cache_invalidate_accepts_tags_and_confirmation_flags() {
        let input = parse([
            "cache".to_string(),
            "invalidate".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--tag".to_string(),
            "route:events.list".to_string(),
            "--tag".to_string(),
            "locale:en-GB".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
        ])
        .unwrap();

        let CliInput::CacheInvalidate {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected cache invalidate input");
        };

        assert_eq!(output_mode, OutputMode::Human);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
        assert_eq!(
            invocation.tags,
            vec!["route:events.list".to_string(), "locale:en-GB".to_string()]
        );
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_tls_status_uses_explicit_config_path() {
        let input = parse([
            "tls".to_string(),
            "status".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::TlsStatus {
            output_mode,
            config_path,
        } = input
        else {
            panic!("expected tls status input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(config_path, PathBuf::from("/tmp/platform.toml"));
    }

    #[test]
    fn parse_jobs_status_accepts_an_optional_queue_filter() {
        let input = parse([
            "jobs".to_string(),
            "status".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--queue".to_string(),
            "jobs.work".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsStatus {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected jobs status input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.queue.as_deref(), Some("jobs.work"));
    }

    #[test]
    fn parse_jobs_run_accepts_queue_worker_limit_and_dry_run() {
        let input = parse([
            "jobs".to_string(),
            "run".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--queue".to_string(),
            "jobs.work".to_string(),
            "--worker-id".to_string(),
            "worker-a".to_string(),
            "--limit".to_string(),
            "5".to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsRun {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected jobs run input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.queue.as_deref(), Some("jobs.work"));
        assert_eq!(invocation.worker_id.as_deref(), Some("worker-a"));
        assert_eq!(invocation.limit, 5);
    }

    #[test]
    fn parse_jobs_ready_accepts_queue_and_limit() {
        let input = parse([
            "jobs".to_string(),
            "ready".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--queue".to_string(),
            "jobs.work".to_string(),
            "--limit".to_string(),
            "25".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsReady {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected jobs ready input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.queue.as_deref(), Some("jobs.work"));
        assert_eq!(invocation.limit, 25);
    }

    #[test]
    fn parse_jobs_dead_letters_accepts_queue_and_limit() {
        let input = parse([
            "jobs".to_string(),
            "dead-letters".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--queue".to_string(),
            "jobs.dead-letter".to_string(),
            "--limit".to_string(),
            "25".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsDeadLetters {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected jobs dead-letters input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.queue.as_deref(), Some("jobs.dead-letter"));
        assert_eq!(invocation.limit, 25);
    }

    #[test]
    fn parse_jobs_in_flight_accepts_queue_worker_and_limit() {
        let input = parse([
            "jobs".to_string(),
            "in-flight".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--queue".to_string(),
            "jobs.work".to_string(),
            "--worker-id".to_string(),
            "worker-a".to_string(),
            "--limit".to_string(),
            "5".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsInFlight {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected jobs in-flight input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.queue.as_deref(), Some("jobs.work"));
        assert_eq!(invocation.worker_id.as_deref(), Some("worker-a"));
        assert_eq!(invocation.limit, 5);
    }

    #[test]
    fn parse_jobs_retry_accepts_dead_letter_id_and_confirmation_flags() {
        let input = parse([
            "jobs".to_string(),
            "retry".to_string(),
            "dead-letter:job-retry".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--dry-run".to_string(),
            "--yes".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsRetry {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected jobs retry input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.dead_letter_id, "dead-letter:job-retry");
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_jobs_promote_accepts_dry_run_and_confirmation_flags() {
        let input = parse([
            "jobs".to_string(),
            "promote".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--dry-run".to_string(),
            "--yes".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::JobsPromote {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected jobs promote input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert!(invocation.confirmed);
    }

    #[test]
    fn parse_jobs_dead_letters_defaults_limit_and_rejects_zero() {
        let input = parse([
            "jobs".to_string(),
            "ready".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
        ])
        .unwrap();

        let CliInput::JobsReady { invocation, .. } = input else {
            panic!("expected jobs ready input");
        };
        assert_eq!(invocation.limit, 50);

        let input = parse([
            "jobs".to_string(),
            "dead-letters".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
        ])
        .unwrap();

        let CliInput::JobsDeadLetters { invocation, .. } = input else {
            panic!("expected jobs dead-letters input");
        };
        assert_eq!(invocation.limit, 50);

        let input = parse([
            "jobs".to_string(),
            "in-flight".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
        ])
        .unwrap();

        let CliInput::JobsInFlight { invocation, .. } = input else {
            panic!("expected jobs in-flight input");
        };
        assert_eq!(invocation.limit, 50);

        let error = parse([
            "jobs".to_string(),
            "dead-letters".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--limit".to_string(),
            "0".to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("greater than zero"));
    }

    #[test]
    fn parse_tls_renew_accepts_certificate_replacement_and_dry_run() {
        let input = parse([
            "tls".to_string(),
            "renew".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
            "--certificate".to_string(),
            "cert-live".to_string(),
            "--replacement".to_string(),
            "cert-next".to_string(),
            "--dry-run".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::TlsRenew {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected tls renew input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
        assert_eq!(invocation.certificate_id, "cert-live");
        assert_eq!(invocation.replacement_certificate_id, "cert-next");
        assert!(!invocation.confirmed);
    }

    #[test]
    fn parse_dev_server_uses_explicit_config_path() {
        let input = parse([
            "dev".to_string(),
            "server".to_string(),
            "--config".to_string(),
            "/tmp/platform.toml".to_string(),
        ])
        .unwrap();

        let CliInput::DevServer { invocation } = input else {
            panic!("expected dev server input");
        };

        assert_eq!(invocation.config_path, PathBuf::from("/tmp/platform.toml"));
    }

    #[test]
    fn parse_config_validate_uses_explicit_config_path() {
        let input = parse([
            "config".to_string(),
            "validate".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::ConfigValidate {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected config validate input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
    }

    #[test]
    fn parse_migrate_apply_accepts_dry_run_and_confirmation_flags() {
        let input = parse([
            "migrate".to_string(),
            "apply".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--dry-run".to_string(),
            "--yes".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::MigrateApply {
            output_mode,
            dry_run,
            invocation,
        } = input
        else {
            panic!("expected migrate apply input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(dry_run);
        assert!(invocation.confirmed);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
    }

    #[test]
    fn parse_assets_publish_accepts_dry_run_and_confirmation_flags() {
        let input = parse([
            "assets".to_string(),
            "publish".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--dry-run".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        let CliInput::AssetsPublish {
            dry_run,
            invocation,
            ..
        } = input
        else {
            panic!("expected assets publish input");
        };

        assert!(dry_run);
        assert!(invocation.confirmed);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
    }

    #[test]
    fn parse_storage_verify_accepts_policy_flag() {
        let input = parse([
            "storage".to_string(),
            "verify".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--policy".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::StorageVerify {
            output_mode,
            config_path,
            verify_policy,
        } = input
        else {
            panic!("expected storage verify input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert!(verify_policy);
        assert_eq!(config_path, PathBuf::from("/tmp/davenda.toml"));
    }

    #[test]
    fn parse_storage_inspect_uses_explicit_config_path() {
        let input = parse([
            "storage".to_string(),
            "inspect".to_string(),
            "--config".to_string(),
            "/tmp/davenda.toml".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::StorageInspect {
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected storage inspect input");
        };

        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.config_path, PathBuf::from("/tmp/davenda.toml"));
    }
}
