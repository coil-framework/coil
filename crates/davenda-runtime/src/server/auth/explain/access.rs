use super::render::render_explanation_json;
use super::*;
use axum::body::Body;
use axum::extract::Request;
use axum::extract::State;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::post;
use axum::Json;
use davenda_auth::{Capability, DefaultSubject, Entity, ExplainOptions, Relation};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use super::super::observability::observability_response;

use crate::LiveAuthExplainRequest;

pub(crate) fn auth_explain_router(
    state: Arc<RuntimeServerState>,
) -> axum::Router<Arc<RuntimeServerState>> {
    if !state.plan.config.auth.explain_api {
        return axum::Router::new();
    }

    let auth_state = state.clone();
    axum::Router::new()
        .route("/diagnostics/auth/explain", post(serve_auth_explain))
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let state = auth_state.clone();
            async move {
                let authorization = match prepare_explain_access(&state, &request) {
                    Ok(check) => authorize_explain_access(&state, check).await,
                    Err(error) => Err(error),
                };
                match authorization {
                    Ok(()) => next.run(request).await,
                    Err(error) => error_response(error),
                }
            }
        }))
}

struct ExplainAccessCheck {
    subject: DefaultSubject,
    object: Entity,
}

fn prepare_explain_access(
    state: &RuntimeServerState,
    request: &Request,
) -> Result<ExplainAccessCheck, RuntimeServerError> {
    let live_request = LiveHttpRequest::from_request(
        request,
        &state.plan.browser,
        &state.plan.config.server,
        None,
    )?;
    let request = live_request.into_request_input()?;
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let resolved = {
        let mut browser = state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        browser
            .resolve_request(&request, &state.cookie_secret, now)
            .map_err(RequestExecutionError::from_browser_error)?
    };

    let Some(principal_id) = resolved.principal_id.as_deref() else {
        return Err(RuntimeServerError::Execution(
            RequestExecutionError::SessionRequired {
                route: "auth.explain".to_string(),
            },
        ));
    };

    Ok(ExplainAccessCheck {
        subject: davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(
            principal_id.to_string(),
        )),
        object: davenda_auth::Entity::admin_module(state.plan.config.app.name.clone()),
    })
}

async fn authorize_explain_access(
    state: &RuntimeServerState,
    check: ExplainAccessCheck,
) -> Result<(), RuntimeServerError> {
    let allowed = state
        .route_authorizer
        .check_capability(
            &check.subject,
            davenda_auth::Capability::AdminAuditRead,
            &check.object,
        )
        .await?;

    if !allowed {
        return Err(RuntimeServerError::Execution(
            RequestExecutionError::CapabilityRequired {
                route: "auth.explain".to_string(),
                capability: davenda_auth::Capability::AdminAuditRead,
            },
        ));
    }

    Ok(())
}

#[derive(Debug)]
enum AuthExplainRequestError {
    BadRequest(String),
    Internal(RuntimeServerError),
}

#[derive(Debug, Deserialize)]
struct AuthExplainRequest {
    subject: String,
    capability: String,
    resource: String,
    #[serde(default)]
    max_depth: Option<usize>,
    #[serde(default)]
    cycle_protection: Option<bool>,
}

async fn serve_auth_explain(
    State(state): State<Arc<RuntimeServerState>>,
    Json(request): Json<AuthExplainRequest>,
) -> Response<Body> {
    match build_auth_explain_response(&state, request).await {
        Ok(value) => observability_response(axum::http::StatusCode::OK, value),
        Err(AuthExplainRequestError::BadRequest(message)) => observability_response(
            axum::http::StatusCode::BAD_REQUEST,
            json!({ "error": message }),
        ),
        Err(AuthExplainRequestError::Internal(error)) => error_response(error),
    }
}

async fn build_auth_explain_response(
    state: &RuntimeServerState,
    request: AuthExplainRequest,
) -> Result<Value, AuthExplainRequestError> {
    let mut options = match request.max_depth {
        Some(max_depth) => ExplainOptions::new(max_depth),
        None => ExplainOptions::default(),
    };
    options = options
        .with_cycle_protection(request.cycle_protection.unwrap_or(true))
        .normalized();

    let request = LiveAuthExplainRequest {
        subject: parse_subject(&request.subject)?,
        capability: parse_capability(&request.capability)?,
        object: parse_entity(&request.resource)?,
        options,
    };

    let explanation = state
        .auth_explainer
        .as_ref()
        .ok_or_else(|| AuthExplainRequestError::Internal(RuntimeServerError::Explain {
            reason: "auth explain API is disabled by deployment config".to_string(),
        }))?
        .explain_capability(&request)
        .await
        .map_err(AuthExplainRequestError::Internal)?;

    Ok(render_explanation_json(state.plan.tenant_id(), &request, &explanation))
}

fn parse_subject(input: &str) -> Result<DefaultSubject, AuthExplainRequestError> {
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

fn parse_entity(input: &str) -> Result<Entity, AuthExplainRequestError> {
    let (namespace, id) = input.split_once(':').ok_or_else(|| {
        AuthExplainRequestError::BadRequest(format!("invalid entity `{input}`"))
    })?;
    if id.trim().is_empty() {
        return Err(AuthExplainRequestError::BadRequest(format!(
            "invalid entity `{input}`"
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
        _ => {
            return Err(AuthExplainRequestError::BadRequest(format!(
                "unknown entity namespace `{namespace}`"
            )));
        }
    };

    Ok(entity)
}

fn parse_relation(input: &str) -> Result<Relation, AuthExplainRequestError> {
    Relation::from_str(input).ok_or_else(|| {
        AuthExplainRequestError::BadRequest(format!("unknown relation `{input}`"))
    })
}

fn parse_capability(input: &str) -> Result<Capability, AuthExplainRequestError> {
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
        _ => {
            return Err(AuthExplainRequestError::BadRequest(format!(
                "unknown capability `{input}`"
            )));
        }
    };

    Ok(capability)
}
