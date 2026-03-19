use super::*;

pub(super) fn required_capabilities() -> Vec<Capability> {
    vec![
        Capability::EventsEventPublish,
        Capability::EventsSlotManage,
        Capability::EventsBookingCreate,
        Capability::EventsBookingCheckIn,
    ]
}

pub(super) fn optional_capabilities() -> Vec<Capability> {
    vec![
        Capability::AdminShellAccess,
        Capability::CmsPageRead,
        Capability::SeoMetadataEdit,
        Capability::I18nTranslationEdit,
        Capability::MembershipSubscriptionManage,
        Capability::AssetRead,
        Capability::CheckoutSessionCreate,
        Capability::OrderRead,
    ]
}

pub(super) fn capability_contracts() -> Vec<CapabilityContract> {
    vec![
        CapabilityContract::required(Capability::EventsEventPublish, ["event"]),
        CapabilityContract::required(Capability::EventsSlotManage, ["event_slot"]),
        CapabilityContract::required(Capability::EventsBookingCreate, ["booking", "event_slot"]),
        CapabilityContract::required(Capability::EventsBookingCheckIn, ["booking"]),
        CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
        CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
        CapabilityContract::optional(Capability::SeoMetadataEdit, ["event"]),
        CapabilityContract::optional(Capability::I18nTranslationEdit, ["event"]),
        CapabilityContract::optional(
            Capability::MembershipSubscriptionManage,
            ["subscription", "membership_tier"],
        ),
        CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
        CapabilityContract::optional(Capability::CheckoutSessionCreate, ["storefront"]),
        CapabilityContract::optional(Capability::OrderRead, ["order"]),
    ]
}

pub(super) fn module_dependencies() -> Vec<ModuleDependency> {
    vec![
        ModuleDependency::optional(
            "admin",
            "Events contributes booking, slot, and check-in resources to the shared admin shell when installed",
        ),
        ModuleDependency::optional(
            "cms",
            "Event pages and discoverability can compose into CMS-driven storefront content",
        ),
        ModuleDependency::optional(
            "commerce",
            "Paid bookings can bridge into checkout and order workflows when commerce is installed",
        ),
        ModuleDependency::optional(
            "memberships",
            "Membership tiers can gate event eligibility and booking access rules",
        ),
    ]
}

pub(super) fn core_service_dependencies() -> Vec<CoreServiceDependency> {
    vec![
        CoreServiceDependency::Auth,
        CoreServiceDependency::Data,
        CoreServiceDependency::Cache,
        CoreServiceDependency::Jobs,
        CoreServiceDependency::I18n,
        CoreServiceDependency::Seo,
        CoreServiceDependency::Template,
        CoreServiceDependency::Observability,
    ]
}
