use crate::cli::error::CliRunError;
use crate::command::OutputMode;
use davenda_auth::{Capability, DefaultSubject, DefaultTuple, Entity, ExplainOptions, Relation};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplainInvocation {
    pub tenant_id: i64,
    pub subject: DefaultSubject,
    pub capability: Capability,
    pub resource: Entity,
    pub tuples: Vec<DefaultTuple>,
    pub options: ExplainOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliInput {
    Help,
    AuthExplain {
        customer_app: String,
        output_mode: OutputMode,
        invocation: AuthExplainInvocation,
    },
}

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<CliInput, CliRunError> {
    let mut customer_app = std::env::var("DAVENDA_APP_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "platform".to_string());
    let mut output_mode = OutputMode::Human;
    let mut tenant_id = 1_i64;
    let mut subject: Option<DefaultSubject> = None;
    let mut capability: Option<Capability> = None;
    let mut resource: Option<Entity> = None;
    let mut tuples = Vec::new();
    let mut max_depth: Option<usize> = None;
    let mut cycle_protection = true;
    let mut positionals = Vec::new();

    let mut iter = args.into_iter();
    while let Some(token) = iter.next() {
        match token.as_str() {
            "--help" | "-h" => return Ok(CliInput::Help),
            "--json" => output_mode = OutputMode::Json,
            "--customer-app" => {
                customer_app = next_value(&mut iter, "--customer-app")?;
            }
            "--tenant-id" => {
                tenant_id = next_value(&mut iter, "--tenant-id")?
                    .parse::<i64>()
                    .map_err(|_| CliRunError::usage("`--tenant-id` must be a valid integer"))?;
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
            "--tuple" => {
                tuples.push(parse_tuple(&next_value(&mut iter, "--tuple")?)?);
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
        [command, subcommand] if command == "auth" && subcommand == "explain" => {
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
                customer_app,
                output_mode,
                invocation: AuthExplainInvocation {
                    tenant_id,
                    subject,
                    capability,
                    resource,
                    tuples,
                    options,
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

fn next_value(
    iter: &mut impl Iterator<Item = String>,
    flag: &'static str,
) -> Result<String, CliRunError> {
    iter.next()
        .ok_or_else(|| CliRunError::usage(format!("`{flag}` expects a value")))
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

fn parse_tuple(input: &str) -> Result<DefaultTuple, CliRunError> {
    let (object, subject) = input.split_once('=').ok_or_else(|| {
        CliRunError::usage(format!(
            "invalid tuple `{input}`; expected object#relation=subject"
        ))
    })?;
    let (object, relation) = object.split_once('#').ok_or_else(|| {
        CliRunError::usage(format!(
            "invalid tuple `{input}`; expected object#relation=subject"
        ))
    })?;

    Ok(DefaultTuple::new(
        parse_entity(object)?,
        parse_relation(relation)?,
        parse_subject(subject)?,
    ))
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
    fn parse_auth_explain_flags_and_positionals() {
        let input = parse([
            "auth".to_string(),
            "explain".to_string(),
            "--subject".to_string(),
            "user:alice".to_string(),
            "--capability".to_string(),
            "cms.page.publish".to_string(),
            "--resource".to_string(),
            "page:homepage".to_string(),
            "--tuple".to_string(),
            "page:homepage#site=site:main".to_string(),
            "--json".to_string(),
        ])
        .unwrap();

        let CliInput::AuthExplain {
            customer_app,
            output_mode,
            invocation,
        } = input
        else {
            panic!("expected auth explain input");
        };
        assert_eq!(customer_app, "platform");
        assert_eq!(output_mode, OutputMode::Json);
        assert_eq!(invocation.tenant_id, 1);
        assert_eq!(invocation.tuples.len(), 1);
        assert_eq!(invocation.resource, Entity::page("homepage"));
    }

    #[test]
    fn parse_help_without_positionals() {
        assert_eq!(parse(Vec::<String>::new()).unwrap(), CliInput::Help);
    }
}
