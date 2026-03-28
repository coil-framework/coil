use super::*;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveAuthorizationCheck {
    pub subject: coil_auth::DefaultSubject,
    pub capability: coil_auth::Capability,
    pub object: coil_auth::Entity,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StaticLiveRouteCapabilityAuthorizer {
    allowed: Vec<LiveAuthorizationCheck>,
    checks: Arc<Mutex<Vec<LiveAuthorizationCheck>>>,
}

impl StaticLiveRouteCapabilityAuthorizer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn allowing(
        mut self,
        subject: coil_auth::DefaultSubject,
        capability: coil_auth::Capability,
        object: coil_auth::Entity,
    ) -> Self {
        self.allowed.push(LiveAuthorizationCheck {
            subject,
            capability,
            object,
        });
        self
    }

    pub(crate) fn checks(&self) -> Vec<LiveAuthorizationCheck> {
        self.checks
            .lock()
            .expect("static live authorizer mutex poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StaticLiveAuthExplainer {
    response: Option<coil_auth::CapabilityExplanation>,
    requests: Arc<Mutex<Vec<LiveAuthExplainRequest>>>,
}

impl StaticLiveAuthExplainer {
    pub(crate) fn new(response: coil_auth::CapabilityExplanation) -> Self {
        Self {
            response: Some(response),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Vec<LiveAuthExplainRequest> {
        self.requests
            .lock()
            .expect("static live explainer mutex poisoned")
            .clone()
    }
}

impl LiveAuthExplainer for StaticLiveAuthExplainer {
    fn explain_capability<'a>(
        &'a self,
        request: &'a LiveAuthExplainRequest,
    ) -> AuthExplainFuture<'a> {
        let response = self.response.clone().expect("static response must be set");
        Box::pin(async move {
            self.requests
                .lock()
                .expect("static live explainer mutex poisoned")
                .push(request.clone());
            Ok(response)
        })
    }
}

impl LiveRouteCapabilityAuthorizer for StaticLiveRouteCapabilityAuthorizer {
    fn check_capability<'a>(
        &'a self,
        subject: &'a coil_auth::DefaultSubject,
        capability: coil_auth::Capability,
        object: &'a coil_auth::Entity,
    ) -> RouteAuthorizationFuture<'a> {
        Box::pin(async move {
            let check = LiveAuthorizationCheck {
                subject: subject.clone(),
                capability,
                object: object.clone(),
            };
            self.checks
                .lock()
                .expect("static live authorizer mutex poisoned")
                .push(check.clone());
            Ok(self.allowed.iter().any(|allowed| allowed == &check))
        })
    }
}
