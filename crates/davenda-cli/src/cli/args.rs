use crate::cli::config::ConfigValidateInvocation;
use crate::cli::error::CliRunError;
use crate::cli::import::ImportRunInvocation;
use crate::command::OutputMode;
use davenda_auth::{Capability, DefaultSubject, Entity, ExplainOptions, Relation};
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
pub(crate) struct DevServerInvocation {
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
    ImportRun {
        output_mode: OutputMode,
        dry_run: bool,
        invocation: ImportRunInvocation,
    },
}

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<CliInput, CliRunError> {
    let mut config_path = std::env::var("DAVENDA_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let mut output_mode = OutputMode::Human;
    let mut dry_run = false;
    let mut subject: Option<DefaultSubject> = None;
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
            "--config" => {
                config_path = Some(PathBuf::from(next_value(&mut iter, "--config")?));
            }
            "--subject" => {
                subject = Some(parse_subject(&next_value(&mut iter, "--subject")?)?);
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
        [command, subcommand, manifest_path] if command == "import" && subcommand == "run" => {
            Ok(CliInput::ImportRun {
                output_mode,
                dry_run,
                invocation: ImportRunInvocation {
                    manifest_path: PathBuf::from(manifest_path),
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

fn parse_subject(input: &str) -> Result<DefaultSubject, CliRunError> {
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

fn parse_entity(input: &str) -> Result<Entity, CliRunError> {
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

fn parse_capability(input: &str) -> Result<Capability, CliRunError> {
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
}
