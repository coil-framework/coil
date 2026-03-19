use super::super::core::CommerceModule;
use davenda_core::{RegistrationError, ServiceRegistry};

pub(super) fn register_module_services(
    module: &CommerceModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name().to_string(),
        "module.commerce.catalog",
        "Catalog products, variants, and collection management",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.commerce.pricing",
        "Pricing policies, discounts, voucher application, and tax/shipping calculation",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.commerce.checkout",
        "Checkout sessions, payment readiness, and order materialization",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.commerce.orders",
        "Order lifecycle, fulfillment outcomes, and refund handling",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.commerce.membership_bridge",
        "Membership-aware product kinds and order outcomes for entitlement modules",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.commerce.admin",
        "Commerce admin resources, catalog operations, and order review",
    )
}
