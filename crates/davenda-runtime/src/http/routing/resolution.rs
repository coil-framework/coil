use davenda_auth::{Entity, Namespace};

pub(super) fn route_capability_resource(
    namespace: Namespace,
    module: Option<&str>,
    contract_kind: Option<&str>,
    route_name: &str,
) -> Entity {
    let resource_id = match (module, contract_kind) {
        (Some(module), Some(contract_kind)) => {
            format!("http.surface.module.{module}.{contract_kind}.{route_name}")
        }
        (Some(module), None) => format!("http.surface.module.{module}.{route_name}"),
        (None, Some(contract_kind)) => format!("http.surface.{contract_kind}.{route_name}"),
        (None, None) => format!("http.surface.{route_name}"),
    };

    match namespace {
        Namespace::Tenant => Entity::tenant(resource_id),
        Namespace::Site => Entity::site(resource_id),
        Namespace::Brand => Entity::brand(resource_id),
        Namespace::Storefront => Entity::storefront(resource_id),
        Namespace::User => Entity::user(resource_id),
        Namespace::Group => Entity::group(resource_id),
        Namespace::Team => Entity::team(resource_id),
        Namespace::ServiceAccount => Entity::service_account(resource_id),
        Namespace::Page => Entity::page(resource_id),
        Namespace::Navigation => Entity::navigation(resource_id),
        Namespace::Product => Entity::product(resource_id),
        Namespace::Collection => Entity::collection(resource_id),
        Namespace::Order => Entity::order(resource_id),
        Namespace::Subscription => Entity::subscription(resource_id),
        Namespace::MembershipTier => Entity::membership_tier(resource_id),
        Namespace::Event => Entity::event(resource_id),
        Namespace::EventSlot => Entity::event_slot(resource_id),
        Namespace::Booking => Entity::booking(resource_id),
        Namespace::Media => Entity::media(resource_id),
        Namespace::MediaLibrary => Entity::media_library(resource_id),
        Namespace::Asset => Entity::asset(resource_id),
        Namespace::AssetFolder => Entity::asset_folder(resource_id),
        Namespace::ThemeAssetBundle => Entity::theme_asset_bundle(resource_id),
        Namespace::AdminModule => Entity::admin_module(resource_id),
    }
}
