use super::*;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventEligibilityRule {
    Public,
    Authenticated,
    RequiresCapability(Capability),
    RequiresAnyCapability(Vec<Capability>),
    RequiresMembershipTierAny(Vec<MembershipTierId>),
    RequiresEntitlementAny(Vec<String>),
    AllOf(Vec<EventEligibilityRule>),
    AnyOf(Vec<EventEligibilityRule>),
}

impl EventEligibilityRule {
    pub fn allows(&self, context: &EventAccessContext) -> Result<(), EligibilityFailure> {
        match self {
            Self::Public => Ok(()),
            Self::Authenticated => {
                if context.authenticated {
                    Ok(())
                } else {
                    Err(EligibilityFailure::AuthenticationRequired)
                }
            }
            Self::RequiresCapability(capability) => {
                if context.granted_capabilities.contains(capability) {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingCapability {
                        capability: *capability,
                    })
                }
            }
            Self::RequiresAnyCapability(capabilities) => {
                if capabilities
                    .iter()
                    .any(|capability| context.granted_capabilities.contains(capability))
                {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingCapability {
                        capability: *capabilities
                            .first()
                            .unwrap_or(&Capability::EventsBookingCreate),
                    })
                }
            }
            Self::RequiresMembershipTierAny(allowed) => {
                if allowed
                    .iter()
                    .any(|tier| context.active_membership_tiers.contains(tier))
                {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingMembershipTier {
                        allowed: allowed.clone(),
                    })
                }
            }
            Self::RequiresEntitlementAny(allowed) => {
                if allowed
                    .iter()
                    .any(|entitlement| context.entitlements.contains(entitlement))
                {
                    Ok(())
                } else {
                    Err(EligibilityFailure::MissingEntitlement {
                        entitlement_key: allowed.first().cloned().unwrap_or_default(),
                    })
                }
            }
            Self::AllOf(rules) => {
                for rule in rules {
                    rule.allows(context)?;
                }
                Ok(())
            }
            Self::AnyOf(rules) => {
                let mut failures = Vec::new();
                for rule in rules {
                    match rule.allows(context) {
                        Ok(()) => return Ok(()),
                        Err(failure) => failures.push(failure),
                    }
                }

                Err(EligibilityFailure::Composite { failures })
            }
        }
    }
}

impl fmt::Display for EventEligibilityRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Authenticated => f.write_str("authenticated"),
            Self::RequiresCapability(capability) => write!(f, "requires capability `{capability}`"),
            Self::RequiresAnyCapability(capabilities) => write!(
                f,
                "requires any of [{}]",
                capabilities
                    .iter()
                    .map(Capability::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RequiresMembershipTierAny(tiers) => write!(
                f,
                "requires any membership tier [{}]",
                tiers
                    .iter()
                    .map(MembershipTierId::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::RequiresEntitlementAny(entitlements) => {
                write!(f, "requires any entitlement [{}]", entitlements.join(", "))
            }
            Self::AllOf(rules) => write!(
                f,
                "all of [{}]",
                rules
                    .iter()
                    .map(EventEligibilityRule::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::AnyOf(rules) => write!(
                f,
                "any of [{}]",
                rules
                    .iter()
                    .map(EventEligibilityRule::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EligibilityFailure {
    AuthenticationRequired,
    MissingCapability { capability: Capability },
    MissingMembershipTier { allowed: Vec<MembershipTierId> },
    MissingEntitlement { entitlement_key: String },
    Composite { failures: Vec<EligibilityFailure> },
}

impl fmt::Display for EligibilityFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthenticationRequired => f.write_str("authentication is required"),
            Self::MissingCapability { capability } => {
                write!(f, "missing capability `{capability}`")
            }
            Self::MissingMembershipTier { allowed } => write!(
                f,
                "missing required membership tier among [{}]",
                allowed
                    .iter()
                    .map(MembershipTierId::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::MissingEntitlement { entitlement_key } => {
                write!(f, "missing entitlement `{entitlement_key}`")
            }
            Self::Composite { failures } => write!(
                f,
                "none of the alternatives matched: {}",
                failures
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAccessContext {
    pub authenticated: bool,
    pub granted_capabilities: HashSet<Capability>,
    pub active_membership_tiers: BTreeSet<MembershipTierId>,
    pub entitlements: BTreeSet<String>,
}

impl EventAccessContext {
    pub fn public() -> Self {
        Self::default()
    }

    pub fn with_authenticated(mut self, authenticated: bool) -> Self {
        self.authenticated = authenticated;
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }

    pub fn with_membership_tier(mut self, tier_id: MembershipTierId) -> Self {
        self.active_membership_tiers.insert(tier_id);
        self
    }

    pub fn with_entitlement(mut self, entitlement: impl Into<String>) -> Self {
        self.entitlements.insert(entitlement.into());
        self
    }
}

impl Default for EventAccessContext {
    fn default() -> Self {
        Self {
            authenticated: false,
            granted_capabilities: HashSet::new(),
            active_membership_tiers: BTreeSet::new(),
            entitlements: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorAccessContext {
    pub granted_capabilities: HashSet<Capability>,
}

impl OperatorAccessContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }

    pub fn allows(&self, capability: Capability) -> bool {
        self.granted_capabilities.contains(&capability)
    }
}

impl Default for OperatorAccessContext {
    fn default() -> Self {
        Self {
            granted_capabilities: HashSet::new(),
        }
    }
}
