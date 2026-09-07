use fission::core::env::RouteLocation;
use fission::{RouteDecision, RouteRedirect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Authenticated identity and the stable Coil capabilities granted to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoilPrincipal {
    pub subject: String,
    pub capabilities: BTreeSet<String>,
}

impl CoilPrincipal {
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            capabilities: BTreeSet::new(),
        }
    }

    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.insert(capability.into());
        self
    }

    pub fn can(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }
}

/// Request-owned authentication state hydrated through a Fission job.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoilSessionState {
    #[default]
    Loading,
    SignedOut,
    Authenticated(CoilPrincipal),
    Failed,
}

/// Central access selector for Fission `ProtectedRoute` instances.
///
/// This controls component construction only. Server jobs, actions, and APIs
/// must authenticate and authorize their own operations independently.
pub fn protected_route_decision(
    session: &CoilSessionState,
    location: &RouteLocation,
    required_capability: &str,
) -> RouteDecision {
    match session {
        CoilSessionState::Loading => RouteDecision::Pending,
        CoilSessionState::Authenticated(principal) if principal.can(required_capability) => {
            RouteDecision::Allow
        }
        CoilSessionState::Authenticated(_) | CoilSessionState::Failed => RouteDecision::Deny,
        CoilSessionState::SignedOut => RouteRedirect::replace("/sign-in")
            .return_to(location)
            .into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_out_navigation_returns_to_the_origin_free_route() {
        let decision = protected_route_decision(
            &CoilSessionState::SignedOut,
            &RouteLocation::from_route("/account/orders?status=open#latest"),
            "account.view",
        );

        assert_eq!(
            decision,
            RouteDecision::Redirect(RouteRedirect::replace(
                "/sign-in?return_to=%2Faccount%2Forders%3Fstatus%3Dopen%23latest"
            ))
        );
    }

    #[test]
    fn capabilities_are_decided_in_one_pure_selector() {
        let principal = CoilPrincipal::new("user:42").with_capability("account.view");
        let location = RouteLocation::new("/account");

        assert_eq!(
            protected_route_decision(
                &CoilSessionState::Authenticated(principal.clone()),
                &location,
                "account.view"
            ),
            RouteDecision::Allow
        );
        assert_eq!(
            protected_route_decision(
                &CoilSessionState::Authenticated(principal),
                &location,
                "account.manage"
            ),
            RouteDecision::Deny
        );
    }
}
