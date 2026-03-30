use super::*;

pub struct MembershipsModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceContribution>,
}

impl MembershipsModule {
    pub fn new() -> Self {
        Self {
            name: "memberships".to_string(),
            config_namespace: "memberships".to_string(),
            admin_resources: vec![
                AdminResourceContribution::new(
                    "memberships.tiers",
                    "/admin/memberships/tiers",
                    "Membership tiers",
                    "Tiers",
                    AdminNavigationSection::Memberships,
                    AdminContributionKind::ResourceIndex,
                    Capability::MembershipTierEdit,
                ),
                AdminResourceContribution::new(
                    "memberships.subscriptions",
                    "/admin/memberships/subscriptions",
                    "Subscriptions",
                    "Subscriptions",
                    AdminNavigationSection::Memberships,
                    AdminContributionKind::ResourceIndex,
                    Capability::MembershipSubscriptionManage,
                ),
                AdminResourceContribution::new(
                    "memberships.passes",
                    "/admin/memberships/passes",
                    "Passes and credits",
                    "Passes",
                    AdminNavigationSection::Memberships,
                    AdminContributionKind::ResourceIndex,
                    Capability::MembershipSubscriptionManage,
                ),
            ],
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config_namespace(&self) -> &str {
        &self.config_namespace
    }

    pub fn admin_resources(&self) -> &[AdminResourceContribution] {
        &self.admin_resources
    }
}

impl Default for MembershipsModule {
    fn default() -> Self {
        Self::new()
    }
}
