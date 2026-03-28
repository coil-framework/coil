use super::*;

pub(super) fn register_module_services(
    module: &MembershipsModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name().to_string(),
        "module.memberships.tiers",
        "Membership tiers, benefits, and plan configuration",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.memberships.subscriptions",
        "Subscription lifecycle, grace periods, pause and cancellation handling",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.memberships.entitlements",
        "Entitlement grants and revocation aligned with auth-backed member access",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.memberships.renewals",
        "Renewal scheduling, retry orchestration, and subscription follow-up work",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.memberships.commerce_bridge",
        "Commerce order outcomes translated into membership subscription state",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.memberships.admin",
        "Membership operator resources for tiers, subscriptions, and entitlement review",
    )
}
