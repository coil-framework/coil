use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventsModule {
    pub(super) name: String,
    pub(super) config_namespace: String,
    pub(super) admin_resources: Vec<AdminResourceContribution>,
}

impl EventsModule {
    pub fn new() -> Self {
        Self {
            name: "events".to_string(),
            config_namespace: "events".to_string(),
            admin_resources: vec![
                AdminResourceContribution::new(
                    "events.events",
                    "/admin/events/events",
                    "Events",
                    "Events",
                    AdminNavigationSection::Events,
                    AdminContributionKind::ResourceIndex,
                    Capability::EventsEventPublish,
                ),
                AdminResourceContribution::new(
                    "events.slots",
                    "/admin/events/slots",
                    "Slots",
                    "Slots",
                    AdminNavigationSection::Events,
                    AdminContributionKind::ResourceIndex,
                    Capability::EventsSlotManage,
                ),
                AdminResourceContribution::new(
                    "events.bookings",
                    "/admin/events/bookings",
                    "Bookings",
                    "Bookings",
                    AdminNavigationSection::Events,
                    AdminContributionKind::ResourceIndex,
                    Capability::EventsBookingCreate,
                ),
                AdminResourceContribution::new(
                    "events.check-in",
                    "/admin/events/check-in",
                    "Check-in",
                    "Check-in",
                    AdminNavigationSection::Events,
                    AdminContributionKind::Workflow,
                    Capability::EventsBookingCheckIn,
                ),
            ],
        }
    }

    pub fn admin_resources(&self) -> &[AdminResourceContribution] {
        &self.admin_resources
    }
}

impl Default for EventsModule {
    fn default() -> Self {
        Self::new()
    }
}
