use super::*;
use crate::builder::RegisteredHookKind;
use crate::storefront::{
    StorefrontCartLine, StorefrontFormState, StorefrontOrderSnapshot, StorefrontPaymentSnapshot,
    StorefrontStateSnapshot, StorefrontStateStore,
};
use coil_assets::AssetDeliveryTarget;
use coil_commerce::{
    CheckoutId, CheckoutLine, CheckoutSession, CurrencyCode, EntitlementKey, Money, Order, OrderId,
    PricingPolicy, ProductId, ProductKind, Sku,
};
use coil_customer_sdk::{
    AuditEntry, AuditFacade, AuthCheckRequest, AuthCheckResult, AuthExplainRequest,
    AuthExplanation, AuthFacade, BackendError, BackendErrorKind, CommerceFacade,
    CustomerAppContext as CustomerPluginAppContext, MoneyAmount, OrderDraft, OrderLineDraft,
    OrderReviewDecision, PrincipalContext as CustomerPluginPrincipalContext,
    PrincipalKind as CustomerPluginPrincipalKind, RenderModelContribution, RenderTarget,
    RepositoryFacade, RepositoryQuery, RepositoryRecord, RepositoryRecordSet, RepositoryWrite,
    RepositoryWriteReceipt, RequestContext as CustomerPluginRequestContext,
    TraceContext as CustomerPluginTraceContext,
};
use coil_memberships::{
    BillingInterval, MemberAccountId, MembershipCatalog, MembershipInstant, MembershipModelError,
    MembershipTier, MembershipTierId, SubscriptionStatus, TierVisibility,
};
use coil_template::{
    RenderModel, RenderModelMergePolicy, RenderValue, TemplateModelError, TemplateNamespace,
    TrustedHtml,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use url::form_urlencoded;

struct RuntimeCustomerCommerceFacade<'a> {
    catalog: &'a StorefrontCatalog,
    order_id: &'a str,
    review_notes: Arc<Mutex<Vec<String>>>,
}

impl CommerceFacade for RuntimeCustomerCommerceFacade<'_> {
    fn product(
        &self,
        sku: &str,
    ) -> Result<Option<coil_customer_sdk::CommerceProduct>, BackendError> {
        Ok(self.catalog.product_by_sku_or_handle(sku).map(|product| {
            coil_customer_sdk::CommerceProduct {
                sku: product.sku.clone(),
                handle: product.handle.clone(),
                title: product.title.clone(),
                current_price: MoneyAmount::new(product.currency.clone(), product.price_minor),
                collection_handle: Some(product.collection_handle.clone()),
                metadata: BTreeMap::new(),
            }
        }))
    }

    fn add_order_note(&self, order_id: &str, note: &str) -> Result<(), BackendError> {
        if order_id != self.order_id {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "commerce.order_note.order_mismatch",
                format!(
                    "Render-time customer review can only annotate the active order `{}`; received `{order_id}`.",
                    self.order_id
                ),
            ));
        }
        let note = note.trim();
        if note.is_empty() {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                "commerce.order_note.empty",
                "Render-time customer review requires a non-empty order note.",
            ));
        }
        let mut notes = self.review_notes.lock().map_err(|_| {
            BackendError::new(
                BackendErrorKind::Internal,
                "commerce.order_note.state_poisoned",
                "Render-time customer review could not record the order note.",
            )
        })?;
        if !notes.iter().any(|existing| existing == note) {
            notes.push(note.to_string());
        }
        Ok(())
    }
}

struct RuntimeCustomerAuthFacade<'a> {
    plan: &'a RuntimePlan,
    principal_id: Option<&'a str>,
}

impl AuthFacade for RuntimeCustomerAuthFacade<'_> {
    fn check_capability(
        &self,
        request: &AuthCheckRequest,
    ) -> Result<AuthCheckResult, BackendError> {
        let capability = parse_customer_capability(request.capability.as_str())?;
        let object = parse_customer_auth_entity(request.object.as_str())?;
        let subject = customer_hook_auth_subject(self.principal_id);
        let data = self.plan.data.clone();
        let tenant_id = self.plan.tenant_id();
        let auth_package = self.plan.auth_package.clone();
        let allowed = run_customer_hook_future(async move {
            let client = data
                .connect_lazy_postgres()
                .map_err(|error| error.to_string())?;
            let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
            let auth = coil_auth::CoilAuth::new(engine, tenant_id);
            auth.check_capability(auth_package.package(), &subject, capability, &object)
                .await
                .map_err(|error| error.to_string())
        })
        .map_err(customer_hook_auth_backend_error)?;

        Ok(AuthCheckResult {
            allowed,
            explanation: (!allowed).then(|| {
                format!(
                    "live auth denied `{}` for `{}`",
                    request.capability, request.object
                )
            }),
        })
    }

    fn explain_denial(
        &self,
        request: &AuthExplainRequest,
    ) -> Result<AuthExplanation, BackendError> {
        if !self.plan.config.auth.explain_api {
            return Err(BackendError::new(
                BackendErrorKind::Unsupported,
                "auth.explain.unavailable",
                "Runtime auth explanations are disabled for this installation.",
            ));
        }
        let capability = parse_customer_capability(request.capability.as_str())?;
        let object = parse_customer_auth_entity(request.object.as_str())?;
        let subject = customer_hook_auth_subject(self.principal_id);
        let config = self.plan.config.clone();
        let data = self.plan.data.clone();
        let auth_package = self.plan.auth_package.clone();
        let explanation = run_customer_hook_future(async move {
            let explainer =
                coil_auth::LiveAuthExplainHost::from_runtime(&config, data, auth_package)
                    .map_err(|error| error.to_string())?;
            explainer
                .explain_capability(&coil_auth::LiveAuthExplainRequest {
                    subject,
                    capability,
                    object,
                    options: coil_auth::ExplainOptions::default(),
                })
                .await
                .map_err(|error| error.to_string())
        })
        .map_err(customer_hook_auth_backend_error)?;

        Ok(AuthExplanation {
            summary: format!(
                "{} `{}` on `{}`",
                if explanation.decision.is_allowed() {
                    "allow"
                } else {
                    "deny"
                },
                explanation.capability.as_str(),
                explanation.object
            ),
            traces: vec![format!("{:?}", explanation.trace)],
        })
    }
}

struct RuntimeCustomerAuditFacade<'a> {
    plan: &'a RuntimePlan,
    principal_id: Option<&'a str>,
}

impl AuditFacade for RuntimeCustomerAuditFacade<'_> {
    fn record(&self, entry: AuditEntry) -> Result<(), BackendError> {
        let mut serializer = form_urlencoded::Serializer::new(String::new());
        serializer
            .append_pair("action", entry.action.as_str())
            .append_pair("resource_kind", entry.resource_kind.as_str())
            .append_pair("resource_id", entry.resource_id.as_str())
            .append_pair("outcome", entry.outcome.as_str());
        if let Some(detail) = entry.detail.as_deref() {
            serializer.append_pair("detail", detail);
        }
        for (key, value) in &entry.metadata {
            serializer.append_pair(&format!("meta.{key}"), value);
        }
        record_admin_audit_entry(
            self.plan,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
            self.principal_id.unwrap_or("anonymous"),
            serializer.finish(),
        )
        .map_err(|reason| {
            BackendError::new(
                BackendErrorKind::Internal,
                "audit.record.failed",
                "Failed to persist the customer hook audit entry during render.",
            )
            .with_detail(reason)
        })
    }
}

struct RuntimeRenderRepositoryFacade<'a> {
    catalog: &'a StorefrontCatalog,
    workspace: Option<Arc<Mutex<CmsAdminWorkspace>>>,
}

impl RepositoryFacade for RuntimeRenderRepositoryFacade<'_> {
    fn read(&self, query: &RepositoryQuery) -> Result<RepositoryRecordSet, BackendError> {
        let records = match query.repository.as_str() {
            "cms.pages" => self
                .workspace
                .as_ref()
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::Unsupported,
                        "repository.read.unsupported",
                        "Render model hooks did not expose a CMS workspace for this request.",
                    )
                })?
                .lock()
                .map_err(|_| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "repository.workspace.lock_failed",
                        "Runtime could not acquire the CMS workspace lock.",
                    )
                })?
                .pages
                .iter()
                .filter(|page| {
                    query.key.as_deref().map_or(true, |key| {
                        page.id == key
                            || page.draft.slug == key
                            || page.live.as_ref().is_some_and(|live| live.slug == key)
                    })
                })
                .map(|page| {
                    let mut fields = BTreeMap::new();
                    fields.insert("title".to_string(), page.draft.title.clone());
                    fields.insert("slug".to_string(), page.draft.slug.clone());
                    fields.insert("summary".to_string(), page.draft.summary.clone());
                    fields.insert("body_html".to_string(), page.draft.body_html.clone());
                    fields.insert("status".to_string(), page.status_label().to_string());
                    if let Some(live_path) = page.live_path() {
                        fields.insert("live_path".to_string(), live_path);
                    }
                    RepositoryRecord {
                        id: page.id.clone(),
                        fields,
                    }
                })
                .collect(),
            "cms.navigation" => self
                .workspace
                .as_ref()
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::Unsupported,
                        "repository.read.unsupported",
                        "Render model hooks did not expose a CMS workspace for this request.",
                    )
                })?
                .lock()
                .map_err(|_| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "repository.workspace.lock_failed",
                        "Runtime could not acquire the CMS workspace lock.",
                    )
                })?
                .navigation
                .iter()
                .enumerate()
                .filter(|(index, item)| {
                    query
                        .key
                        .as_deref()
                        .map_or(true, |key| key == index.to_string() || key == item.href)
                })
                .map(|(index, item)| RepositoryRecord {
                    id: index.to_string(),
                    fields: BTreeMap::from([
                        ("label".to_string(), item.label.clone()),
                        ("href".to_string(), item.href.clone()),
                    ]),
                })
                .collect(),
            "cms.redirects" => self
                .workspace
                .as_ref()
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::Unsupported,
                        "repository.read.unsupported",
                        "Render model hooks did not expose a CMS workspace for this request.",
                    )
                })?
                .lock()
                .map_err(|_| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "repository.workspace.lock_failed",
                        "Runtime could not acquire the CMS workspace lock.",
                    )
                })?
                .redirects
                .iter()
                .enumerate()
                .filter(|(index, redirect)| {
                    query
                        .key
                        .as_deref()
                        .map_or(true, |key| key == index.to_string() || key == redirect.from)
                })
                .map(|(index, redirect)| RepositoryRecord {
                    id: index.to_string(),
                    fields: BTreeMap::from([
                        ("from".to_string(), redirect.from.clone()),
                        ("to".to_string(), redirect.to.clone()),
                        ("permanent".to_string(), redirect.permanent.to_string()),
                    ]),
                })
                .collect(),
            "commerce.catalog.products" => self
                .catalog
                .products
                .iter()
                .filter(|product| {
                    query
                        .key
                        .as_deref()
                        .map_or(true, |key| product.handle == key || product.sku == key)
                        && query
                            .filters
                            .get("collection_handle")
                            .map_or(true, |handle| product.collection_handle == *handle)
                })
                .map(|product| RepositoryRecord {
                    id: product.handle.clone(),
                    fields: BTreeMap::from([
                        ("handle".to_string(), product.handle.clone()),
                        ("sku".to_string(), product.sku.clone()),
                        ("title".to_string(), product.title.clone()),
                        ("summary".to_string(), product.summary.clone()),
                        ("price_minor".to_string(), product.price_minor.to_string()),
                        ("currency".to_string(), product.currency.clone()),
                        (
                            "collection_handle".to_string(),
                            product.collection_handle.clone(),
                        ),
                        ("is_visible".to_string(), product.is_visible.to_string()),
                        ("product_kind".to_string(), product.product_kind.clone()),
                        (
                            "entitlement_key".to_string(),
                            product.entitlement_key.clone().unwrap_or_default(),
                        ),
                    ]),
                })
                .collect(),
            "commerce.catalog.collections" => self
                .catalog
                .collections
                .iter()
                .filter(|collection| {
                    query
                        .key
                        .as_deref()
                        .map_or(true, |key| collection.handle == key)
                })
                .map(|collection| RepositoryRecord {
                    id: collection.handle.clone(),
                    fields: BTreeMap::from([
                        ("handle".to_string(), collection.handle.clone()),
                        ("title".to_string(), collection.title.clone()),
                        ("label".to_string(), collection.label.clone()),
                        ("summary".to_string(), collection.summary.clone()),
                        ("is_visible".to_string(), collection.is_visible.to_string()),
                    ]),
                })
                .collect(),
            "commerce.orders" => {
                return Err(BackendError::new(
                    BackendErrorKind::Unsupported,
                    "repository.read.unsupported",
                    "Render model hooks do not expose commerce order reads during template rendering.",
                ));
            }
            _ => {
                return Err(BackendError::new(
                    BackendErrorKind::Unsupported,
                    "repository.read.unsupported",
                    format!(
                        "Render model hooks only expose `cms.pages`, `cms.navigation`, `cms.redirects`, `commerce.catalog.products`, and `commerce.catalog.collections` reads; `{}` is not available.",
                        query.repository
                    ),
                ));
            }
        };

        Ok(RepositoryRecordSet {
            repository: query.repository.clone(),
            records,
        })
    }

    fn write(&self, change: RepositoryWrite) -> Result<RepositoryWriteReceipt, BackendError> {
        Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "repository.write.unsupported",
            format!(
                "Render model hooks are read-only; repository `{}` cannot be written during render.",
                change.repository
            ),
        ))
    }
}

impl RuntimePlan {
    pub(super) fn template_namespaces_for_execution(
        &self,
        execution: &RequestExecution,
    ) -> Vec<TemplateNamespace> {
        let module_namespace = self.module_template_namespace(execution);
        self.template.namespace_chain(module_namespace.as_ref())
    }

    pub(super) fn module_template_namespace(
        &self,
        execution: &RequestExecution,
    ) -> Option<TemplateNamespace> {
        self.http
            .routes
            .iter()
            .find(|route| route.name == execution.route.route_name)
            .and_then(|route| route.module.as_deref())
            .and_then(|module| TemplateNamespace::new(module.to_string()).ok())
    }

    pub(crate) fn render_model_for_execution(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        fragment_id: Option<&str>,
    ) -> Result<RenderModel, TemplateModelError> {
        let storefront_feedback = storefront_page_feedback(
            execution.route.route_name.as_str(),
            &execution.flash_messages,
        );
        let mut model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(execution.customer_app.clone()),
            )?
            .with_value(
                "route_name",
                RenderValue::text(execution.route.route_name.clone()),
            )?
            .with_value("path", RenderValue::text(execution.path.clone()))?
            .with_value("locale", RenderValue::text(execution.locale.clone()))?
            .with_value(
                "method",
                RenderValue::text(format!("{:?}", execution.method)),
            )?
            .with_value(
                "template_name",
                RenderValue::text(template_name.to_string()),
            )?
            .with_value(
                "route_area",
                RenderValue::text(format!("{:?}", execution.route_area)),
            )?
            .with_value(
                "request_id",
                RenderValue::text(execution.trace.request_id.clone()),
            )?
            .with_value(
                "transport_scheme",
                RenderValue::text(execution.trace.transport_scheme.clone()),
            )?
            .with_value(
                "principal_id",
                RenderValue::text(
                    execution
                        .principal
                        .principal_id
                        .clone()
                        .unwrap_or_else(|| "anonymous".to_string()),
                ),
            )?
            .with_value(
                "session_id",
                RenderValue::text(
                    execution
                        .session
                        .session_id
                        .clone()
                        .unwrap_or_else(|| "guest".to_string()),
                ),
            )?
            .with_value(
                "surface_id",
                RenderValue::text(
                    fragment_id
                        .map(str::to_string)
                        .unwrap_or_else(|| execution.route.route_name.clone()),
                ),
            )?
            .with_object("site", site_model(self, execution)?)?
            .with_object("route_params", route_params_model(&execution.route.params))?
            .with_object("links", links_model(self, execution)?)?
            .with_object("navigation", navigation_model(Some(self))?)?
            .with_bool(
                "has_flash_messages",
                !storefront_feedback.visible_flash_messages.is_empty(),
            )?
            .with_list(
                "flash_messages",
                flash_messages_model(&storefront_feedback.visible_flash_messages)?,
            )?
            .with_object(
                "page",
                page_model_for_route(execution, template_name, fragment_id),
            )?
            .with_bool(
                "has_linked_customer_plugins",
                !self.linked_customer_plugins.is_empty(),
            )?
            .with_list(
                "linked_customer_plugins",
                linked_customer_plugins_model(&self.linked_customer_plugins)?,
            )?;

        if let Some(fragment_id) = fragment_id {
            model = model.with_value("fragment_id", RenderValue::text(fragment_id.to_string()))?;
        }

        if let Some(manifest) = &self.theme_asset_manifest {
            for (logical_path, published) in manifest.entries() {
                if let AssetDeliveryTarget::Cdn { public_url, .. } = published.delivery().target() {
                    model = model.with_asset_path(logical_path, public_url.clone())?;
                }
            }
        }

        let locale_context = self.i18n.request_context(Some(execution.locale.as_str()));
        for (key, value) in self.i18n.translations.resolved_messages(&locale_context) {
            model = model.with_translation(key.as_str(), value)?;
        }

        let model = apply_route_specific_bindings(
            Some(self),
            model,
            execution.route.route_name.as_str(),
            execution.site_id.as_deref(),
            execution.locale.as_str(),
            &execution.route.params,
            &execution.query_params,
            storefront_feedback.form_state.as_ref(),
            Some(&execution.session),
            Some(&execution.principal),
        )?;

        apply_customer_render_model_contributions(self, execution, template_name, fragment_id, model)
    }
}

fn linked_customer_plugins_model(
    plugins: &[crate::builder::LinkedCustomerPluginSummary],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    plugins
        .iter()
        .map(|plugin| {
            RenderModel::new()
                .with_value("id", RenderValue::text(plugin.plugin_id.clone()))?
                .with_value(
                    "display_name",
                    RenderValue::text(plugin.display_name.clone()),
                )?
                .with_value("version", RenderValue::text(plugin.version.clone()))?
                .with_value(
                    "hooks_summary",
                    RenderValue::text(
                        plugin
                            .registered_hooks
                            .iter()
                            .map(registered_hook_label)
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                )
        })
        .collect()
}

fn registered_hook_label(kind: &RegisteredHookKind) -> &'static str {
    match kind {
        RegisteredHookKind::Checkout => "checkout",
        RegisteredHookKind::CmsPagePublish => "cms-page-publish",
        RegisteredHookKind::RenderModel => "render-model",
        RegisteredHookKind::VerifiedWebhook => "verified-webhook",
        RegisteredHookKind::VerifiedWebhookAssets => "verified-webhook-assets",
    }
}

fn apply_customer_render_model_contributions(
    plan: &RuntimePlan,
    execution: &RequestExecution,
    template_name: &str,
    fragment_id: Option<&str>,
    mut model: RenderModel,
) -> Result<RenderModel, TemplateModelError> {
    if plan.customer_hooks.render_model.is_empty() {
        return Ok(model);
    }

    let workspace = cms_admin_workspace(plan)
        .ok()
        .map(|workspace| Arc::new(Mutex::new(workspace)));
    let repositories = RuntimeRenderRepositoryFacade {
        catalog: &plan.storefront_catalog,
        workspace,
    };
    let audit = RuntimeCustomerAuditFacade {
        plan,
        principal_id: execution.principal.principal_id.as_deref(),
    };
    let context = runtime_customer_render_request_context(plan, execution);
    let target = runtime_customer_render_target(execution, template_name, fragment_id);

    for hook in &plan.customer_hooks.render_model {
        let contributions = hook
            .contribute_render_model(&context, &target, &repositories, &audit)
            .map_err(customer_plugin_template_error)?;
        for contribution in contributions {
            model = apply_customer_render_model_contribution(model, contribution)?;
        }
    }

    Ok(model)
}

fn apply_customer_render_model_contribution(
    model: RenderModel,
    contribution: RenderModelContribution,
) -> Result<RenderModel, TemplateModelError> {
    match contribution {
        RenderModelContribution::Mount { path, model: value } => {
            model.mount_object(path.as_str(), value)
        }
        RenderModelContribution::Merge {
            path,
            model: value,
            policy,
        } => model.merge_object(path.as_str(), value, merge_policy(policy)),
    }
}

fn merge_policy(policy: coil_customer_sdk::MergePolicy) -> RenderModelMergePolicy {
    match policy {
        coil_customer_sdk::MergePolicy::FailOnConflict => {
            RenderModelMergePolicy::FailOnConflict
        }
        coil_customer_sdk::MergePolicy::ReplaceExisting => {
            RenderModelMergePolicy::ReplaceExisting
        }
        coil_customer_sdk::MergePolicy::AppendLists => RenderModelMergePolicy::AppendLists,
    }
}

fn runtime_customer_render_request_context(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> CustomerPluginRequestContext {
    let environment = match plan.config.app.environment {
        coil_config::Environment::Development => "development",
        coil_config::Environment::Staging => "staging",
        coil_config::Environment::Production => "production",
    };
    let mut customer_app =
        CustomerPluginAppContext::new(execution.customer_app.clone(), environment.to_owned());
    if let Some(site_id) = execution.site_id.as_deref() {
        customer_app = customer_app.with_site_id(site_id.to_string());
    }
    if !execution.locale.trim().is_empty() {
        customer_app = customer_app.with_locale(execution.locale.clone());
    }
    let principal = customer_plugin_principal(execution.principal.principal_id.as_deref());
    let trace = CustomerPluginTraceContext::new(execution.trace.request_id.clone())
        .with_request_id(execution.trace.request_id.clone());
    CustomerPluginRequestContext::new(customer_app, principal, trace)
}

fn runtime_customer_render_target(
    execution: &RequestExecution,
    template_name: &str,
    fragment_id: Option<&str>,
) -> RenderTarget {
    let mut target = RenderTarget::new(
        execution.route.route_name.clone(),
        template_name.to_string(),
        execution.locale.clone(),
    )
    .with_route_params(execution.route.params.clone())
    .with_query_params(
        execution
            .query_params
            .iter()
            .filter_map(|(key, values)| values.first().map(|value| (key.clone(), value.clone())))
            .collect(),
    );
    if let Some(site_id) = execution.site_id.as_deref() {
        target = target.with_site_id(site_id.to_string());
    }
    if let Some(fragment_id) = fragment_id {
        target = target.with_fragment_id(fragment_id.to_string());
    }
    target
}

fn route_params_model(params: &BTreeMap<String, String>) -> RenderModel {
    let mut model = RenderModel::new();
    for (key, value) in params {
        model = model
            .with_value(key.clone(), RenderValue::text(value.clone()))
            .expect("route params are validated tokens");
    }
    model
}

fn navigation_model(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new().with_list("primary", primary_navigation_items(plan)?)
}

fn nav_item(label: &str, href: &str) -> RenderModel {
    RenderModel::new()
        .with_value("label", RenderValue::text(label))
        .and_then(|model| model.with_value("href", RenderValue::text(href)))
        .expect("navigation item keys are valid")
}

fn primary_navigation_items(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let items = if let Some(plan) = plan {
        cms_admin_workspace(plan)?
            .navigation
            .into_iter()
            .map(|item| nav_item(item.label.as_str(), item.href.as_str()))
            .collect::<Vec<_>>()
    } else {
        vec![
            nav_item("Home", "/"),
            nav_item("Shop", "/shop"),
            nav_item("Collections", "/shop/collections"),
            nav_item("Events", "/events"),
            nav_item("Cart", "/cart"),
            nav_item("Account", "/account"),
        ]
    };
    Ok(items)
}

fn site_model(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> Result<RenderModel, TemplateModelError> {
    let canonical_host = plan
        .config
        .canonical_host_for_site(execution.site_id.as_deref())
        .to_string();
    let display_name = execution
        .site_display_name
        .clone()
        .unwrap_or_else(|| execution.customer_app.clone());
    let brand_name = execution
        .brand_name
        .clone()
        .unwrap_or_else(|| display_name.clone());
    let site_id = execution
        .site_id
        .clone()
        .unwrap_or_else(|| "default".to_string());

    RenderModel::new()
        .with_value("id", RenderValue::text(site_id))?
        .with_value("display_name", RenderValue::text(display_name))?
        .with_value("request_host", RenderValue::text(execution.host.clone()))?
        .with_value("canonical_host", RenderValue::text(canonical_host))?
        .with_bool("has_brand_name", true)?
        .with_value("brand_name", RenderValue::text(brand_name))
}

fn links_model(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> Result<RenderModel, TemplateModelError> {
    let site_id = execution.site_id.as_deref();
    let locale = execution.locale.as_str();
    RenderModel::new()
        .with_value(
            "home",
            RenderValue::text(route_link(
                plan,
                site_id,
                "home",
                &BTreeMap::new(),
                Some(locale),
                "/",
            )),
        )?
        .with_value(
            "catalog",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.catalog",
                &BTreeMap::new(),
                Some(locale),
                &localized_shop_path(locale),
            )),
        )?
        .with_value(
            "collections",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.collections",
                &BTreeMap::new(),
                Some(locale),
                &localized_collections_path(locale),
            )),
        )?
        .with_value(
            "featured_collection",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.collection-detail",
                &BTreeMap::from([("collection_slug".to_string(), "featured".to_string())]),
                Some(locale),
                &localized_collection_path(locale, "featured"),
            )),
        )?
        .with_value(
            "memberships_collection",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.collection-detail",
                &BTreeMap::from([("collection_slug".to_string(), "memberships".to_string())]),
                Some(locale),
                &localized_collection_path(locale, "memberships"),
            )),
        )?
        .with_value(
            "events_collection",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.collection-detail",
                &BTreeMap::from([("collection_slug".to_string(), "events".to_string())]),
                Some(locale),
                &localized_collection_path(locale, "events"),
            )),
        )?
        .with_value(
            "cart",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.cart",
                &BTreeMap::new(),
                None,
                "/cart",
            )),
        )?
        .with_value(
            "checkout",
            RenderValue::text(route_link(
                plan,
                site_id,
                "commerce.checkout",
                &BTreeMap::new(),
                None,
                "/checkout",
            )),
        )?
        .with_list("locale_switches", locale_switches_model(plan, execution)?)?
        .with_list("site_switches", site_switches_model(plan, execution)?)?
        .with_value("account", RenderValue::text("/account"))?
        .with_value("orders", RenderValue::text("/account/orders"))?
        .with_value("memberships", RenderValue::text("/account/memberships"))?
        .with_value("admin_dashboard", RenderValue::text("/admin"))?
        .with_value("admin_audit", RenderValue::text("/admin/audit"))?
        .with_value("admin_orders", RenderValue::text("/admin/orders"))?
        .with_value("admin_catalog", RenderValue::text("/admin/catalog/products"))?
        .with_value("admin_pages", RenderValue::text("/admin/pages"))?
        .with_value("admin_navigation", RenderValue::text("/admin/navigation"))?
        .with_value("admin_redirects", RenderValue::text("/admin/redirects"))
}

fn route_link(
    plan: &RuntimePlan,
    site_id: Option<&str>,
    route_name: &str,
    params: &BTreeMap<String, String>,
    locale: Option<&str>,
    fallback: &str,
) -> String {
    plan.http
        .path_for_site(&plan.config, site_id, route_name, params, locale)
        .unwrap_or_else(|_| fallback.to_string())
}

fn locale_switches_model(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let site_id = execution.site_id.as_deref();
    let route_name = execution.route.route_name.as_str();
    let params = &execution.route.params;
    let current_locale = execution.locale.as_str();
    let route_localized = route_uses_localized_paths(plan, route_name);

    plan.config
        .supported_locales_for_site(site_id)
        .iter()
        .map(|locale| {
            let href = if route_localized || locale == current_locale {
                route_link(
                    plan,
                    site_id,
                    route_name,
                    params,
                    Some(locale.as_str()),
                    &locale_root_fallback_path(&plan.config, site_id, locale.as_str()),
                )
            } else {
                locale_root_fallback_path(&plan.config, site_id, locale.as_str())
            };
            RenderModel::new()
                .with_value("id", RenderValue::text(locale.clone()))?
                .with_value(
                    "label",
                    RenderValue::text(locale_display_label(locale.as_str())),
                )?
                .with_value("href", RenderValue::text(href))?
                .with_bool("active", locale == current_locale)
        })
        .collect()
}

fn site_switches_model(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let route_name = execution.route.route_name.as_str();
    let params = &execution.route.params;
    let current_site_id = execution.site_id.as_deref();
    let current_locale = execution.locale.as_str();
    let scheme = execution.trace.transport_scheme.as_str();
    let route_localized = route_uses_localized_paths(plan, route_name);

    plan.config
        .sites
        .iter()
        .map(|site| {
            let target_locale = if site
                .supported_locales
                .iter()
                .any(|candidate| candidate == current_locale)
            {
                current_locale
            } else {
                site.default_locale.as_str()
            };
            let target_host = host_with_request_port(
                site.canonical_host.as_str(),
                execution.host.as_str(),
            );
            let href = if route_localized || current_site_id == Some(site.id.as_str()) {
                plan.http
                    .path_for_site(
                        &plan.config,
                        Some(site.id.as_str()),
                        route_name,
                        params,
                        Some(target_locale),
                    )
                    .map(|path| format!("{scheme}://{target_host}{path}"))
                    .unwrap_or_else(|_| {
                        format!(
                            "{scheme}://{target_host}{}",
                            locale_root_fallback_path(
                                &plan.config,
                                Some(site.id.as_str()),
                                target_locale,
                            )
                        )
                    })
            } else {
                format!(
                    "{scheme}://{target_host}{}",
                    locale_root_fallback_path(
                        &plan.config,
                        Some(site.id.as_str()),
                        target_locale,
                    )
                )
            };

            RenderModel::new()
                .with_value("id", RenderValue::text(site.id.clone()))?
                .with_value("label", RenderValue::text(site.display_name.clone()))?
                .with_value("href", RenderValue::text(href))?
                .with_value("locale", RenderValue::text(target_locale.to_string()))?
                .with_bool("active", current_site_id == Some(site.id.as_str()))
        })
        .collect()
}

fn route_uses_localized_paths(plan: &RuntimePlan, route_name: &str) -> bool {
    plan.http
        .routes
        .iter()
        .find(|route| route.name == route_name)
        .map(|route| route.locale_policy == LocalePolicy::Localized)
        .unwrap_or(false)
}

fn locale_display_label(locale: &str) -> &'static str {
    match locale {
        "en-GB" => "English",
        "fr-FR" => "Français",
        "pl-PL" => "Polski",
        "de-DE" => "Deutsch",
        _ => "Locale",
    }
}

fn locale_root_fallback_path(
    config: &PlatformConfig,
    site_id: Option<&str>,
    locale: &str,
) -> String {
    let localized_routes = config.localized_routes_for_site(site_id);
    if !localized_routes {
        return "/".to_string();
    }

    if locale == config.default_locale_for_site(site_id) {
        "/".to_string()
    } else {
        format!("/{}", locale.trim_matches('/'))
    }
}

fn host_with_request_port(canonical_host: &str, request_host: &str) -> String {
    if canonical_host.contains(':') {
        return canonical_host.to_string();
    }

    let port = match request_host.rsplit_once(':') {
        Some((candidate, port))
            if !candidate.is_empty()
                && !candidate.contains(':')
                && port.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            Some(port)
        }
        _ => None,
    };

    match port {
        Some(port) => format!("{canonical_host}:{port}"),
        None => canonical_host.to_string(),
    }
}

fn page_model_for_route(
    execution: &RequestExecution,
    template_name: &str,
    fragment_id: Option<&str>,
) -> RenderModel {
    let brand_name = execution.brand_name.as_deref().unwrap_or("Shoppr");
    let title = match execution.route.route_name.as_str() {
        "home" => brand_name.to_string(),
        "commerce.catalog" => format!("Shop {brand_name}"),
        "commerce.collections" => "Shop Collections".to_string(),
        "commerce.collection-detail" => execution
            .route
            .params
            .get("collection_slug")
            .map(|slug| title_case_handle(slug))
            .unwrap_or_else(|| "Collection".to_string()),
        "commerce.product-detail" => execution
            .route
            .params
            .get("product_slug")
            .map(|slug| title_case_handle(slug))
            .unwrap_or_else(|| "Product".to_string()),
        "commerce.cart" => "Cart".to_string(),
        "commerce.checkout" => "Checkout".to_string(),
        "commerce.checkout-confirmation" => "Order Confirmed".to_string(),
        "commerce.account.orders" => "Order History".to_string(),
        "admin.dashboard" => format!("{brand_name} Admin"),
        "admin.audit" => "Audit Log".to_string(),
        "commerce.orders" => "Orders".to_string(),
        "commerce.order-detail" => execution
            .route
            .params
            .get("order_id")
            .map(|order_id| format!("Order {order_id}"))
            .unwrap_or_else(|| "Order Detail".to_string()),
        "commerce.catalog-admin" => "Catalog Administration".to_string(),
        "cms.pages.index" => "Pages".to_string(),
        "cms.navigation.index" => "Navigation".to_string(),
        "cms.redirects.index" => "Redirects".to_string(),
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Your Account".to_string()
        }
        _ => execution.route.route_name.clone(),
    };

    let summary = match execution.route.route_name.as_str() {
        "commerce.catalog" => {
            "Browse the current assortment across apparel, memberships, and event-linked offers."
        }
        "commerce.collections" => {
            "Browse curated collection landings before moving into product detail and checkout."
        }
        "commerce.collection-detail" => {
            "A merchandising collection page with clear paths into products and checkout."
        }
        "commerce.product-detail" => {
            "Product detail, pricing, and purchase intent in the HTML-first storefront flow."
        }
        "commerce.cart" => "Review the basket before moving into checkout.",
        "commerce.checkout" => {
            "Confirm contact, delivery, and payment details before finalization."
        }
        "commerce.checkout-confirmation" => {
            "The customer-facing confirmation step after successful checkout."
        }
        "commerce.account.orders" => {
            "Review completed purchases, payment details, and membership-linked order history."
        }
        "admin.dashboard" => {
            "Operator dashboard for launch-day catalog, orders, and content checks."
        }
        "admin.audit" => {
            "Recent privileged actions, the acting operator, and the affected resources."
        }
        "commerce.orders" => {
            "Store-wide order queue with payment state, customer email, and refund visibility."
        }
        "commerce.order-detail" => {
            "Support and finance detail for a specific order, including payment, customer, and refund state."
        }
        "commerce.catalog-admin" => {
            "Live catalog copy, storefront visibility, list price, and collection management for the checked-in store."
        }
        "cms.pages.index" => "Draft, preview, publish, and unpublish live CMS pages.",
        "cms.navigation.index" => {
            "Edit the live primary navigation links rendered in the storefront shell."
        }
        "cms.redirects.index" => {
            "Manage live redirect rules for legacy and unmatched storefront routes."
        }
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Membership state, recent orders, and next actions for the signed-in customer."
        }
        _ => "Server-rendered storefront and account surface.",
    };

    RenderModel::new()
        .with_value("title", RenderValue::text(title))
        .and_then(|model| model.with_value("summary", RenderValue::text(summary)))
        .and_then(|model| model.with_value("template", RenderValue::text(template_name)))
        .and_then(|model| {
            model.with_value("fragment_mode", RenderValue::bool(fragment_id.is_some()))
        })
        .expect("page model keys are valid")
}

fn apply_route_specific_bindings(
    plan: Option<&RuntimePlan>,
    mut model: RenderModel,
    route_name: &str,
    site_id: Option<&str>,
    locale: &str,
    params: &BTreeMap<String, String>,
    query_params: &RequestFieldMap,
    form_state: Option<&StorefrontFormState>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<RenderModel, TemplateModelError> {
    let effective_catalog = effective_storefront_catalog(plan)?;
    let catalog = &effective_catalog;
    let fixture = storefront_fixture(locale, site_id, catalog, plan)?;

    match route_name {
        "home" | "commerce.catalog" | "commerce.collections" => {
            let has_catalog_sections = !fixture.catalog_sections.is_empty();
            let has_product_cards = !fixture.product_cards.is_empty();
            model = model
                .with_bool("has_catalog_sections", has_catalog_sections)?
                .with_bool("has_product_cards", has_product_cards)?
                .with_list("catalog_sections", fixture.catalog_sections.clone())?
                .with_list("product_cards", fixture.product_cards.clone())?;
        }
        "commerce.collection-detail" => {
            let slug = params
                .get("collection_slug")
                .map(String::as_str)
                .unwrap_or("featured");
            if let Some(_collection) = catalog.visible_collection_for_site(site_id, slug) {
                let product_cards = fixture.product_cards_for_collection(slug);
                model = model
                    .with_bool("has_collection", true)?
                    .with_object("collection", fixture.collection_for(slug))?
                    .with_bool("has_product_cards", !product_cards.is_empty())?
                    .with_list("product_cards", product_cards)?;
            } else {
                model = model
                    .with_bool("has_collection", false)?
                    .with_bool("has_product_cards", false)?
                    .with_list("product_cards", Vec::<RenderModel>::new())?
                    .with_value(
                        "missingCollectionHandle",
                        RenderValue::text(slug.to_string()),
                    )?;
            }
        }
        "commerce.product-detail" => {
            let slug = params
                .get("product_slug")
                .map(String::as_str)
                .unwrap_or("harbor-cap");
            if catalog.visible_product_for_site(site_id, slug).is_some() {
                let product_cards = fixture.related_product_cards_for_product(slug);
                model = model
                    .with_bool("has_product", true)?
                    .with_object("product", fixture.product_for(slug))?
                    .with_bool("has_product_cards", !product_cards.is_empty())?
                    .with_list("product_cards", product_cards)?;
            } else {
                model = model
                    .with_bool("has_product", false)?
                    .with_bool("has_product_cards", false)?
                    .with_list("product_cards", Vec::<RenderModel>::new())?
                    .with_value("missing_product_handle", RenderValue::text(slug.to_string()))?;
            }
        }
        "commerce.cart" => {
            if let Some(snapshot) = live_storefront_state(plan, session, principal)? {
                model = model
                    .with_bool("has_cart_items", !snapshot.cart.lines.is_empty())?
                    .with_list(
                        "cart_items",
                        cart_items_from_storefront(
                            catalog,
                            locale,
                            &snapshot.cart.lines,
                            form_state,
                        )?,
                    )?
                    .with_object("cart_summary", cart_summary_from_storefront(&snapshot)?)?
                    .with_object("cart_form", cart_form_model(form_state)?)?;
            } else {
                model = model
                    .with_bool("has_cart_items", !fixture.cart_items.is_empty())?
                    .with_list("cart_items", fixture.cart_items.clone())?
                    .with_object("cart_summary", fixture.cart_summary.clone())?
                    .with_object("cart_form", cart_form_model(form_state)?)?;
            }
        }
        "commerce.checkout" => {
            if let Some(snapshot) = live_storefront_state(plan, session, principal)? {
                let line_items =
                    cart_items_from_storefront(catalog, locale, &snapshot.cart.lines, form_state)?;
                model = model
                    .with_object("customer", checkout_customer(principal)?)?
                    .with_object(
                        "checkout",
                        checkout_form_from_storefront(
                            plan,
                            &snapshot.payment,
                            principal,
                            form_state,
                        )?,
                    )?
                    .with_bool("has_line_items", !line_items.is_empty())?
                    .with_list("line_items", line_items)?
                    .with_object("order_summary", cart_summary_from_storefront(&snapshot)?)?;
            } else {
                let checkout = merge_checkout_form_feedback(fixture.checkout.clone(), form_state)?;
                model = model
                    .with_object("customer", fixture.customer.clone())?
                    .with_object("checkout", checkout)?
                    .with_bool("has_line_items", !fixture.cart_items.is_empty())?
                    .with_list("line_items", fixture.cart_items.clone())?
                    .with_object("order_summary", fixture.cart_summary.clone())?;
            }
        }
        "commerce.checkout-confirmation" => {
            let account =
                account_surface_bindings(plan, &fixture, locale, session, principal, true)?;
            if let Some(snapshot) = live_storefront_state(plan, session, principal)? {
                let confirmation = snapshot
                    .latest_order
                    .as_ref()
                    .map(|order| confirmation_from_storefront(plan, order))
                    .transpose()?
                    .unwrap_or(empty_confirmation_model(plan)?);
                if snapshot.latest_order.is_some() {
                    model = model
                        .with_bool("has_confirmation", true)?
                        .with_object("confirmation", confirmation)?
                        .with_object("account", account.account)?
                        .with_object("customer", account.customer)?
                        .with_list("recent_orders", account.recent_orders)?
                        .with_object("membership_summary", account.membership_summary)?;
                } else {
                    model = model
                        .with_bool("has_confirmation", false)?
                        .with_object("confirmation", confirmation)?
                        .with_object("account", account.account)?
                        .with_object("customer", account.customer)?
                        .with_list("recent_orders", account.recent_orders)?
                        .with_object("membership_summary", account.membership_summary)?;
                }
            } else {
                model = model
                    .with_bool("has_confirmation", true)?
                    .with_object("confirmation", fixture.confirmation.clone())?
                    .with_object("account", account.account)?
                    .with_object("customer", account.customer)?
                    .with_list("recent_orders", account.recent_orders)?
                    .with_object("membership_summary", account.membership_summary)?;
            }
        }
        "commerce.account.orders"
        | "memberships.account"
        | "memberships.account.dashboard"
        | "account.dashboard" => {
            let include_pending_membership = route_name == "memberships.account";
            let account = account_surface_bindings(
                plan,
                &fixture,
                locale,
                session,
                principal,
                include_pending_membership,
            )?;
            model = model
                .with_object("account", account.account)?
                .with_object("customer", account.customer)?
                .with_list("recent_orders", account.recent_orders)?
                .with_object("membership_summary", account.membership_summary)?;
        }
        "admin.dashboard" => {
            let live_recent_orders = recent_orders_from_storefront(
                live_storefront_state(plan, session, principal)?.as_ref(),
            )?;
            let order_count = if live_recent_orders.is_empty() {
                fixture.recent_orders.len()
            } else {
                live_recent_orders.len()
            };
            let content_count = if let Some(plan) = plan {
                cms_admin_workspace(plan)?.pages.len().to_string()
            } else {
                content_pages(locale)?.len().to_string()
            };
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_admin_panels", true)?
                .with_list("admin_panels", admin_panels(locale, &fixture, order_count)?)?
                .with_value(
                    "catalog_count",
                    RenderValue::text(fixture.product_cards.len().to_string()),
                )?
                .with_value("order_count", RenderValue::text(order_count.to_string()))?
                .with_value("content_count", RenderValue::text(content_count))?;
        }
        "admin.audit" => {
            let audit_history = admin_audit_history(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_audit_entries", !audit_history.entries.is_empty())?
                .with_list("audit_entries", audit_history.entries)?
                .with_value(
                    "audit_empty_text",
                    RenderValue::text(audit_history.empty_text),
                )?
                .with_value("audit_backend", RenderValue::text(audit_history.backend))?
                .with_value("audit_location", RenderValue::text(audit_history.location))?
                .with_value(
                    "audit_entry_count",
                    RenderValue::text(audit_history.entry_count.to_string()),
                )?;
        }
        "commerce.orders" => {
            let (recent_orders, order_stats) = admin_orders_from_storefront(plan, &fixture)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_recent_orders", !recent_orders.is_empty())?
                .with_list("recent_orders", recent_orders)?
                .with_object("order_stats", order_stats)?
                .with_value(
                    "orders_empty_text",
                    RenderValue::text(
                        "No completed orders have been captured in the checked-in sample app yet.",
                    ),
                )?;
        }
        "commerce.order-detail" => {
            let (recent_orders, _) = admin_orders_from_storefront(plan, &fixture)?;
            let selected_order = params
                .get("order_id")
                .and_then(|order_id| {
                    order_detail_from_storefront(plan, order_id, form_state, session, principal)
                        .transpose()
                })
                .transpose()?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_recent_orders", !recent_orders.is_empty())?
                .with_list("recent_orders", recent_orders)?
                .with_bool("has_selected_order", selected_order.is_some())?
                .with_bool(
                    "has_missing_order",
                    params.get("order_id").is_some() && selected_order.is_none(),
                )?;
            if let Some(order) = selected_order {
                model = model.with_object("selected_order", order)?;
            }
            model = merge_order_refund_form_feedback(model, form_state)?;
        }
        "commerce.catalog-admin" => {
            let product_cards = catalog_admin_products_model(locale, catalog, plan, form_state)?;
            let catalog_sections = catalog_admin_collections_model(locale, catalog, form_state)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_object("catalog_admin_form", catalog_admin_form_model(form_state)?)?
                .with_bool("has_catalog_sections", !catalog_sections.is_empty())?
                .with_list("catalog_sections", catalog_sections)?
                .with_bool("has_product_cards", !product_cards.is_empty())?
                .with_list("product_cards", product_cards)?
                .with_value(
                    "catalog_empty_text",
                    RenderValue::text("No catalog entries are available in the sample app yet."),
                )?;
        }
        "cms.pages.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let pages = cms_admin_pages_model(&workspace)?;
            let is_creating_page =
                query_flag(query_params, "new") || cms_admin_form_targets_new_page(form_state);
            let selected_page = (!is_creating_page)
                .then(|| {
                    workspace
                        .selected_page(query_first(query_params, "page"))
                        .cloned()
                })
                .flatten();
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_content_pages", !pages.is_empty())?
                .with_list("content_pages", pages)?
                .with_bool(
                    "has_selected_content_page",
                    selected_page.is_some() || is_creating_page,
                )?
                .with_bool("is_creating_content_page", is_creating_page)?
                .with_bool("has_persisted_content_page", selected_page.is_some())?
                .with_object(
                    "selected_content_page",
                    cms_admin_selected_page_model_with_form_state(selected_page, form_state)?,
                )?
                .with_value(
                    "new_content_page_href",
                    RenderValue::text("/admin/pages?new=1"),
                )?
                .with_value(
                    "content_page_editor_title",
                    RenderValue::text(if is_creating_page {
                        "New page draft"
                    } else {
                        "Draft workflow"
                    }),
                )?
                .with_value(
                    "content_page_editor_summary",
                    RenderValue::text(if is_creating_page {
                        "Start a new draft page, save it to create a stable preview, then publish it when the route is ready."
                    } else {
                        "Update the draft, review the preview, then publish it to the live route."
                    }),
                )?
                .with_value(
                    "content_page_save_label",
                    RenderValue::text(if is_creating_page {
                        "Create draft"
                    } else {
                        "Save draft"
                    }),
                )?
                .with_bool(
                    "show_pages_empty_state",
                    workspace.pages.is_empty() && !is_creating_page,
                )?
                .with_value(
                    "pages_empty_text",
                    RenderValue::text(
                        "Create or update a draft page, preview it below, then publish it to the live /pages/{slug} route.",
                    ),
                )?;
            model = merge_cms_page_form_feedback(model, form_state)?;
        }
        "cms.preview" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let selected_page = workspace
                .selected_page(query_first(query_params, "page"))
                .cloned();
            model = model.with_object(
                "selected_content_page",
                cms_admin_selected_page_model_with_form_state(selected_page, form_state)?,
            )?;
        }
        "cms.navigation.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_navigation_items", !workspace.navigation.is_empty())?
                .with_list(
                    "navigation_items",
                    cms_navigation_items_model(&workspace.navigation)?,
                )?
                .with_value(
                    "navigationEmptyText",
                    RenderValue::text("Add at least one primary navigation item before saving."),
                )?;
            model = merge_cms_navigation_form_feedback(model, form_state)?;
        }
        "cms.redirects.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_redirects", !workspace.redirects.is_empty())?
                .with_list("redirects", cms_redirects_model(&workspace.redirects)?)?
                .with_value(
                    "redirectsEmptyText",
                    RenderValue::text(
                        "Add redirect rules for unmatched legacy URLs before cutover.",
                    ),
                )?;
            model = merge_cms_redirect_form_feedback(model, form_state)?;
        }
        "cms.page" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let slug = params.get("slug").map(String::as_str).unwrap_or_default();
            model = model.with_object("cms_page", cms_live_page_model(&workspace, slug)?)?;
        }
        _ => {}
    }

    Ok(model)
}

fn effective_storefront_catalog(
    plan: Option<&RuntimePlan>,
) -> Result<StorefrontCatalog, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(StorefrontCatalog::default_sample());
    };
    StorefrontStateStore::open_for_plan(plan)
        .map_err(template_store_error)?
        .catalog()
        .map_err(template_store_error)
}

fn live_storefront_state(
    plan: Option<&RuntimePlan>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<StorefrontStateSnapshot>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(None);
    };
    let Some(session_id) = session.and_then(|session| session.session_id.as_deref()) else {
        return Ok(None);
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let snapshot = store
        .snapshot(
            session_id,
            principal.and_then(|ctx| ctx.principal_id.as_deref()),
        )
        .map_err(template_store_error)?;
    Ok(Some(snapshot))
}

fn live_storefront_latest_order(
    plan: Option<&RuntimePlan>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<StorefrontOrderSnapshot>, TemplateModelError> {
    Ok(live_storefront_state(plan, session, principal)?.and_then(|snapshot| snapshot.latest_order))
}

fn cart_items_from_storefront(
    catalog: &StorefrontCatalog,
    locale: &str,
    lines: &[StorefrontCartLine],
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    lines
        .iter()
        .map(|line| cart_item_from_storefront(catalog, locale, line, form_state))
        .collect::<Result<Vec<_>, _>>()
}

fn cart_summary_from_storefront(
    snapshot: &StorefrontStateSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value(
            "subtotal",
            RenderValue::text(snapshot.cart.subtotal.clone()),
        )?
        .with_value("shipping", RenderValue::text("£0.00"))?
        .with_value("total", RenderValue::text(snapshot.cart.subtotal.clone()))
}

fn confirmation_from_storefront(
    plan: Option<&RuntimePlan>,
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    let includes_membership = order
        .lines
        .iter()
        .any(|line| line.product_kind == "membership");
    let payment_is_final = matches!(order.status.as_str(), "paid" | "fulfilled");
    let next_step = if !payment_is_final {
        configured_payment_provider(plan)
            .map(|provider| provider.pending_next_step())
            .unwrap_or_else(|| {
                "Payment confirmation is pending. The order will move forward after the provider callback arrives.".to_string()
            })
    } else if includes_membership {
        "A confirmation email and membership activation will follow shortly.".to_string()
    } else {
        "A confirmation email and fulfillment summary are on the way.".to_string()
    };
    let payment_method = payment_method_label(order.payment.method.as_deref());
    let payment_status = payment_status_label(&order.payment.status);
    let payment_summary = payment_summary(
        order.payment.method.as_deref(),
        order.payment.last4.as_deref(),
        order.payment.reference.as_deref(),
    );
    RenderModel::new()
        .with_value("order_number", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "email",
            RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
        )?
        .with_bool("has_email", order.payment.checkout_email.is_some())?
        .with_value("next_step", RenderValue::text(next_step))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )?
        .with_value("subtotal", RenderValue::text(order.subtotal.clone()))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value("payment_status", RenderValue::text(payment_status))?
        .with_value("payment_method", RenderValue::text(payment_method))?
        .with_value(
            "payment_reference",
            RenderValue::text(order.payment.reference.clone().unwrap_or_default()),
        )?
        .with_value(
            "payment_last4",
            RenderValue::text(order.payment.last4.clone().unwrap_or_default()),
        )?
        .with_value("payment_summary", RenderValue::text(payment_summary))?
        .with_value(
            "provider_label",
            RenderValue::text(payment_provider_label(plan)),
        )?
        .with_bool("has_payment_last4", order.payment.last4.is_some())?
        .with_bool("has_payment_reference", order.payment.reference.is_some())?
        .with_bool("has_membership_items", includes_membership)?
        .with_bool("has_line_items", !order.lines.is_empty())?
        .with_list("line_items", confirmation_line_items_from_storefront(order)?)
}

fn empty_confirmation_model(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("order_number", RenderValue::text(String::new()))?
        .with_value("status", RenderValue::text("No recent order".to_string()))?
        .with_value("total", RenderValue::text("£0.00".to_string()))?
        .with_bool("has_email", false)?
        .with_value("email", RenderValue::text(String::new()))?
        .with_value(
            "next_step",
            RenderValue::text(
                "There is no recent checkout confirmation for this browser session yet.",
            ),
        )?
        .with_value(
            "provider_label",
            RenderValue::text(payment_provider_label(plan)),
        )?
        .with_value(
            "payment_summary",
            RenderValue::text("No payment has been submitted yet."),
        )?
        .with_bool("has_line_items", false)?
        .with_list("line_items", Vec::new())
}

fn account_order_from_storefront(
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    let payment_summary = payment_summary(
        order.payment.method.as_deref(),
        order.payment.last4.as_deref(),
        order.payment.reference.as_deref(),
    );
    RenderModel::new()
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )
        .and_then(|model| {
            model.with_value("line_count", RenderValue::text(order.line_count.to_string()))
        })
        .and_then(|model| {
            model.with_value(
                "checkout_email",
                RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
            )
        })
        .and_then(|model| model.with_value("payment_summary", RenderValue::text(payment_summary)))
        .and_then(|model| {
            model.with_bool("has_checkout_email", order.payment.checkout_email.is_some())
        })
        .and_then(|model| {
            model.with_bool(
                "has_payment_summary",
                order.payment.method.is_some()
                    || order.payment.reference.is_some()
                    || order.payment.last4.is_some(),
            )
        })
}

fn admin_orders_from_storefront(
    plan: Option<&RuntimePlan>,
    _fixture: &StorefrontFixture,
) -> Result<(Vec<RenderModel>, RenderModel), TemplateModelError> {
    let Some(plan) = plan else {
        return Ok((Vec::new(), admin_order_stats(0, 0, 0)?));
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let orders = store.admin_orders(50).map_err(template_store_error)?;
    if orders.is_empty() {
        Ok((Vec::new(), admin_order_stats(0, 0, 0)?))
    } else {
        let pending = orders
            .iter()
            .filter(|order| order.status == "pending_payment")
            .count();
        let refunded = orders
            .iter()
            .filter(|order| order.status == "refunded")
            .count();
        let rows = orders
            .iter()
            .map(admin_order_row_from_storefront)
            .collect::<Result<Vec<_>, _>>()?;
        Ok((rows, admin_order_stats(orders.len(), pending, refunded)?))
    }
}

fn order_detail_from_storefront(
    plan: Option<&RuntimePlan>,
    order_id: &str,
    form_state: Option<&StorefrontFormState>,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(None);
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let Some(order) = store.admin_order(order_id).map_err(template_store_error)? else {
        return Ok(None);
    };
    let payment_summary = payment_summary(
        order.payment.method.as_deref(),
        order.payment.last4.as_deref(),
        order.payment.reference.as_deref(),
    );
    let can_refund = matches!(
        order.status.as_str(),
        "paid" | "fulfilled" | "partially_refunded"
    ) && order.refundable_total_minor > 0;
    let refund_reason = form_state
        .and_then(|state| state.fields.get("reason"))
        .cloned()
        .unwrap_or_else(|| "customer_support".to_string());
    let refund_reason_error = storefront_field_error(form_state, "reason");
    let refund_action_summary = if can_refund {
        "Issue a full remaining refund from the checked-in order state. The order will move to Refunded."
            .to_string()
    } else if order.status == "pending_payment" {
        "This order is still awaiting provider confirmation. Confirm payment capture or failure before refunding."
            .to_string()
    } else if order.refundable_total_minor <= 0 {
        "This order has no remaining refundable balance in the checked-in order state.".to_string()
    } else {
        "This order is not currently eligible for an additional refund from the checked-in workflow."
            .to_string()
    };
    let can_fulfill = order.status == "paid";
    let fulfillment_action_summary = if can_fulfill {
        "Mark the order fulfilled once packing or dispatch is complete. The support queue and order history will show the fulfilled status after this action."
            .to_string()
    } else if order.status == "fulfilled" {
        "This order is already marked fulfilled in the checked-in workflow.".to_string()
    } else if order.status == "pending_payment" {
        "Capture payment before marking the order fulfilled.".to_string()
    } else {
        "This order is not currently eligible for fulfillment in the checked-in workflow."
            .to_string()
    };
    let payment_reference = order.payment.reference.clone().unwrap_or_default();
    let checkout_email = order.payment.checkout_email.clone().unwrap_or_default();
    let principal_id = order.principal_id.clone().unwrap_or_default();
    let line_items = order
        .lines
        .iter()
        .map(|line| {
            RenderModel::new()
                .with_value("title", RenderValue::text(line.title.clone()))?
                .with_value(
                    "variant_title",
                    RenderValue::text(line.variant_title.clone()),
                )?
                .with_value("sku", RenderValue::text(line.sku.clone()))?
                .with_value("quantity", RenderValue::text(line.quantity.to_string()))?
                .with_value("total", RenderValue::text(line.total.clone()))?
                .with_bool(
                    "has_entitlement_key",
                    line.entitlement_key
                        .as_deref()
                        .is_some_and(|value| !value.is_empty()),
                )?
                .with_value(
                    "entitlement_key",
                    RenderValue::text(line.entitlement_key.clone().unwrap_or_default()),
                )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let refunds = order
        .refunds
        .iter()
        .map(|refund| {
            RenderModel::new()
                .with_value("refund_id", RenderValue::text(refund.refund_id.clone()))?
                .with_value("amount", RenderValue::text(refund.amount.clone()))?
                .with_value("reason", RenderValue::text(refund.reason.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let customer_review = customer_order_review(plan, &order, session, principal)?;
    let has_customer_review = customer_review.is_some();
    let customer_review_model = customer_review
        .map(customer_order_review_model)
        .transpose()?;
    let mut model = RenderModel::new()
        .with_value("order_id", RenderValue::text(order.order_id.clone()))?
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )?
        .with_value(
            "payment_status",
            RenderValue::text(payment_status_label(&order.payment.status)),
        )?
        .with_value("payment_summary", RenderValue::text(payment_summary))?
        .with_value("payment_reference", RenderValue::text(payment_reference))?
        .with_bool("has_payment_reference", order.payment.reference.is_some())?
        .with_value("checkout_email", RenderValue::text(checkout_email))?
        .with_bool("has_checkout_email", order.payment.checkout_email.is_some())?
        .with_value("principal_id", RenderValue::text(principal_id))?
        .with_bool("has_principal_id", order.principal_id.is_some())?
        .with_bool("can_fulfill", can_fulfill)?
        .with_value(
            "fulfillment_action_summary",
            RenderValue::text(fulfillment_action_summary),
        )?
        .with_value("session_id", RenderValue::text(order.session_id.clone()))?
        .with_value("subtotal", RenderValue::text(order.subtotal.clone()))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value(
            "refunded_total",
            RenderValue::text(order.refunded_total.clone()),
        )?
        .with_value(
            "refundable_total",
            RenderValue::text(order.refundable_total.clone()),
        )?
        .with_bool("can_refund", can_refund)?
        .with_value(
            "refund_action_summary",
            RenderValue::text(refund_action_summary),
        )?
        .with_value("refund_reason", RenderValue::text(refund_reason))?
        .with_bool("has_refund_reason_error", refund_reason_error.is_some())?
        .with_value(
            "refund_reason_error",
            RenderValue::text(refund_reason_error.unwrap_or_default()),
        )?
        .with_bool("has_refunds", !order.refunds.is_empty())?
        .with_list("refunds", refunds)?
        .with_bool("has_line_items", !order.lines.is_empty())?
        .with_list("line_items", line_items)?
        .with_bool("has_customer_review", has_customer_review)?
        .with_value(
            "detail_href",
            RenderValue::text(format!("/admin/orders/{}", order.order_id)),
        )?;
    if let Some(customer_review_model) = customer_review_model {
        model = model.with_object("customer_review", customer_review_model)?;
    }
    Ok(Some(model))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CustomerOrderReviewOutcome {
    decision: OrderReviewDecision,
    notes: Vec<String>,
}

fn customer_order_review(
    plan: &RuntimePlan,
    order: &StorefrontOrderSnapshot,
    _session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<Option<CustomerOrderReviewOutcome>, TemplateModelError> {
    if plan.customer_hooks.checkout.is_empty() {
        return Ok(None);
    }
    let replay_principal_id = customer_replay_principal_id(order, principal);
    let request = CustomerPluginRequestContext::new(
        CustomerPluginAppContext::new(
            plan.config.app.name.clone(),
            format!("{:?}", plan.config.app.environment).to_ascii_lowercase(),
        ),
        customer_plugin_principal(replay_principal_id),
        CustomerPluginTraceContext::new(format!("order-detail:{}", order.order_id)),
    );
    let review_notes = Arc::new(Mutex::new(Vec::new()));
    let commerce = RuntimeCustomerCommerceFacade {
        catalog: &plan.storefront_catalog,
        order_id: &order.order_id,
        review_notes: Arc::clone(&review_notes),
    };
    let auth = RuntimeCustomerAuthFacade {
        plan,
        principal_id: replay_principal_id,
    };
    let audit = RuntimeCustomerAuditFacade {
        plan,
        principal_id: replay_principal_id,
    };
    let order = customer_order_draft(order, &plan.storefront_catalog);
    let mut adjustment_messages = Vec::new();
    let mut adjustment_metadata = BTreeMap::new();

    for hook in &plan.customer_hooks.checkout {
        match hook
            .review_order(&request, &order, &commerce, &auth, &audit)
            .map_err(customer_plugin_template_error)?
        {
            OrderReviewDecision::Approved => {}
            OrderReviewDecision::Adjusted(adjustment) => {
                adjustment_messages.push(adjustment.reason);
                adjustment_metadata.extend(adjustment.metadata);
            }
            OrderReviewDecision::Rejected(rejection) => {
                return Ok(Some(CustomerOrderReviewOutcome {
                    decision: OrderReviewDecision::Rejected(rejection),
                    notes: customer_review_notes(&review_notes)?,
                }));
            }
        }
    }

    let decision = if adjustment_messages.is_empty() {
        OrderReviewDecision::Approved
    } else {
        OrderReviewDecision::Adjusted(
            coil_customer_sdk::OrderAdjustment::new(adjustment_messages.join("; "))
                .with_metadata_entries(adjustment_metadata),
        )
    };

    Ok(Some(CustomerOrderReviewOutcome {
        decision,
        notes: customer_review_notes(&review_notes)?,
    }))
}

fn customer_order_draft(
    order: &StorefrontOrderSnapshot,
    catalog: &StorefrontCatalog,
) -> OrderDraft {
    let subtotal = MoneyAmount::new(order.currency.clone(), order.subtotal_minor);
    let total = MoneyAmount::new(order.currency.clone(), order.total_minor);
    let lines = order
        .lines
        .iter()
        .map(|line| OrderLineDraft {
            sku: line.sku.clone(),
            title: line.title.clone(),
            quantity: line.quantity,
            unit_price: MoneyAmount::new(line.currency.clone(), line.unit_price_minor),
            product_kind: line.product_kind.clone(),
            collection_handle: catalog
                .product_by_sku_or_handle(&line.sku)
                .map(|product| product.collection_handle.clone()),
            entitlement_key: line.entitlement_key.clone(),
            metadata: line.metadata.clone(),
        })
        .collect();
    let mut metadata = order.metadata.clone();
    metadata
        .entry("session_id".to_string())
        .or_insert_with(|| order.session_id.clone());
    metadata
        .entry("payment_method".to_string())
        .or_insert_with(|| {
            order
                .payment
                .method
                .clone()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        });
    if let Some(checkout_email) = order.payment.checkout_email.as_ref() {
        metadata
            .entry("checkout_email".to_string())
            .or_insert_with(|| checkout_email.trim().to_string());
    }
    if let Some(principal_id) = order.principal_id.as_ref() {
        metadata
            .entry("order_principal_id".to_string())
            .or_insert_with(|| principal_id.clone());
    }
    OrderDraft {
        order_id: order.order_id.clone(),
        currency_code: order.currency.clone(),
        subtotal,
        total,
        lines,
        metadata,
    }
}

fn customer_replay_principal_id<'a>(
    order: &'a StorefrontOrderSnapshot,
    principal: Option<&'a PrincipalContext>,
) -> Option<&'a str> {
    order
        .principal_id
        .as_deref()
        .or_else(|| principal.and_then(|principal| principal.principal_id.as_deref()))
}

fn customer_plugin_principal(principal_id: Option<&str>) -> CustomerPluginPrincipalContext {
    if let Some(principal_id) = principal_id {
        return CustomerPluginPrincipalContext::user(principal_id.to_string());
    }
    CustomerPluginPrincipalContext {
        kind: CustomerPluginPrincipalKind::Anonymous,
        id: None,
    }
}

fn customer_plugin_template_error(error: BackendError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "linked customer hook".to_string(),
        message: error.to_string(),
    }
}

fn run_customer_hook_future<T>(
    future: impl Future<Output = Result<T, String>> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle)
            if matches!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::MultiThread
            ) =>
        {
            tokio::task::block_in_place(|| handle.block_on(future))
        }
        Ok(_) => std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?
                .block_on(future)
        })
        .join()
        .map_err(|_| "customer hook runtime bridge thread panicked".to_string())?,
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(future),
    }
}

fn customer_hook_auth_backend_error(reason: String) -> BackendError {
    BackendError::new(
        BackendErrorKind::Unavailable,
        "auth.live_check.failed",
        "Runtime could not complete the linked customer auth check.",
    )
    .with_detail(reason)
}

fn parse_customer_capability(value: &str) -> Result<coil_auth::Capability, BackendError> {
    coil_auth::Capability::from_str(value).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.capability.invalid",
            format!("Unknown capability `{value}`."),
        )
    })
}

fn parse_customer_auth_entity(value: &str) -> Result<coil_auth::Entity, BackendError> {
    let Some((namespace, id)) = value.split_once(':') else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Invalid auth object `{value}`."),
        ));
    };
    if id.trim().is_empty() {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Invalid auth object `{value}`."),
        ));
    }
    match namespace {
        "tenant" => Ok(coil_auth::Entity::tenant(id)),
        "site" => Ok(coil_auth::Entity::site(id)),
        "brand" => Ok(coil_auth::Entity::brand(id)),
        "storefront" => Ok(coil_auth::Entity::storefront(id)),
        "user" => Ok(coil_auth::Entity::user(id)),
        "group" => Ok(coil_auth::Entity::group(id)),
        "team" => Ok(coil_auth::Entity::team(id)),
        "service_account" => Ok(coil_auth::Entity::service_account(id)),
        "page" => Ok(coil_auth::Entity::page(id)),
        "navigation" => Ok(coil_auth::Entity::navigation(id)),
        "product" => Ok(coil_auth::Entity::product(id)),
        "collection" => Ok(coil_auth::Entity::collection(id)),
        "order" => Ok(coil_auth::Entity::order(id)),
        "subscription" => Ok(coil_auth::Entity::subscription(id)),
        "membership_tier" => Ok(coil_auth::Entity::membership_tier(id)),
        "event" => Ok(coil_auth::Entity::event(id)),
        "event_slot" => Ok(coil_auth::Entity::event_slot(id)),
        "booking" => Ok(coil_auth::Entity::booking(id)),
        "media" => Ok(coil_auth::Entity::media(id)),
        "media_library" => Ok(coil_auth::Entity::media_library(id)),
        "asset" => Ok(coil_auth::Entity::asset(id)),
        "asset_folder" => Ok(coil_auth::Entity::asset_folder(id)),
        "theme_asset_bundle" => Ok(coil_auth::Entity::theme_asset_bundle(id)),
        "admin_module" => Ok(coil_auth::Entity::admin_module(id)),
        _ => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "auth.object.invalid",
            format!("Unknown auth object namespace `{namespace}`."),
        )),
    }
}

fn customer_hook_auth_subject(principal_id: Option<&str>) -> coil_auth::DefaultSubject {
    match principal_id {
        Some(principal_id) => coil_auth::DefaultSubject::entity(coil_auth::Entity::user(
            principal_id.to_string(),
        )),
        None => coil_auth::DefaultSubject::entity(coil_auth::Entity::any_user()),
    }
}

fn customer_review_notes(
    review_notes: &Arc<Mutex<Vec<String>>>,
) -> Result<Vec<String>, TemplateModelError> {
    review_notes
        .lock()
        .map(|notes| notes.clone())
        .map_err(|_| TemplateModelError::TemplateRead {
            path: "linked customer hook".to_string(),
            message: "customer review note state was poisoned".to_string(),
        })
}

fn customer_order_review_model(
    review: CustomerOrderReviewOutcome,
) -> Result<RenderModel, TemplateModelError> {
    let notes = review
        .notes
        .iter()
        .map(|note| RenderModel::new().with_value("text", RenderValue::text(note.clone())))
        .collect::<Result<Vec<_>, _>>()?;
    let adjustment_metadata = match &review.decision {
        OrderReviewDecision::Adjusted(adjustment) => adjustment
            .metadata
            .iter()
            .map(|(key, value)| {
                RenderModel::new()
                    .with_value("key", RenderValue::text(key.clone()))?
                    .with_value("value", RenderValue::text(value.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => Vec::new(),
    };
    let base = match review.decision {
        OrderReviewDecision::Approved => RenderModel::new()
            .with_value("status", RenderValue::text("Approved"))?
            .with_value(
                "summary",
                RenderValue::text(
                    "The linked Harbor customer backend approved this order without extra handling.",
                ),
            )?
            .with_value("code", RenderValue::text("approved"))?
            .with_bool("is_approved", true)?
            .with_bool("is_rejected", false)?
            .with_bool("is_adjusted", false)?,
        OrderReviewDecision::Rejected(rejection) => RenderModel::new()
            .with_value("status", RenderValue::text("Rejected"))?
            .with_value("summary", RenderValue::text(rejection.message.clone()))?
            .with_value("code", RenderValue::text(rejection.code))?
            .with_bool("is_approved", false)?
            .with_bool("is_rejected", true)?
            .with_bool("is_adjusted", false)?,
        OrderReviewDecision::Adjusted(adjustment) => RenderModel::new()
            .with_value("status", RenderValue::text("Adjusted"))?
            .with_value("summary", RenderValue::text(adjustment.reason.clone()))?
            .with_value("code", RenderValue::text("adjusted"))?
            .with_value(
                "assigned_queue",
                RenderValue::text(
                    adjustment
                        .metadata
                        .get("assigned_queue")
                        .cloned()
                        .unwrap_or_default(),
                ),
            )?
            .with_value(
                "service_level",
                RenderValue::text(
                    adjustment
                        .metadata
                        .get("service_level")
                        .cloned()
                        .unwrap_or_default(),
                ),
            )?
            .with_bool(
                "has_assigned_queue",
                adjustment.metadata.contains_key("assigned_queue"),
            )?
            .with_bool(
                "has_service_level",
                adjustment.metadata.contains_key("service_level"),
            )?
            .with_bool("is_approved", false)?
            .with_bool("is_rejected", false)?
            .with_bool("is_adjusted", true)?,
    };
    base.with_bool("has_notes", !notes.is_empty())?
        .with_bool("has_metadata", !adjustment_metadata.is_empty())?
        .with_value("note_count", RenderValue::text(notes.len().to_string()))?
        .with_list("metadata_entries", adjustment_metadata)?
        .with_list("notes", notes)
}

fn admin_order_row_from_storefront(
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "status",
            RenderValue::text(display_status_label(&order.status)),
        )?
        .with_value(
            "payment_status",
            RenderValue::text(payment_status_label(&order.payment.status)),
        )?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_value(
            "customer_email",
            RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
        )?
        .with_bool("has_customer_email", order.payment.checkout_email.is_some())?
        .with_value(
            "detail_href",
            RenderValue::text(format!("/admin/orders/{}", order.order_id)),
        )
}

fn admin_order_stats(
    total: usize,
    pending: usize,
    refunded: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("pending", RenderValue::text(pending.to_string()))?
        .with_value("refunded", RenderValue::text(refunded.to_string()))
}

fn operator_identity(
    principal: Option<&PrincipalContext>,
    session: Option<&SessionContext>,
) -> Result<RenderModel, TemplateModelError> {
    let principal_id = principal
        .and_then(|principal| principal.principal_id.as_deref())
        .unwrap_or_default();
    let display_name = if principal_id.is_empty() {
        "Current Operator".to_string()
    } else {
        display_name_from_principal_id(principal_id)
    };
    RenderModel::new()
        .with_value("display_name", RenderValue::text(display_name))?
        .with_value("principal_id", RenderValue::text(principal_id.to_string()))?
        .with_bool("has_principal", principal.is_some())?
        .with_bool(
            "has_session",
            session
                .and_then(|session| session.session_id.as_deref())
                .is_some(),
        )
}

struct AdminAuditHistoryView {
    entries: Vec<RenderModel>,
    empty_text: String,
    backend: String,
    location: String,
    entry_count: usize,
}

#[derive(Default)]
struct ParsedOperatorAuditRecord {
    action: String,
    route: String,
    capability: String,
    resource_kind: String,
    resource_id: String,
    outcome: String,
    detail: String,
}

fn admin_audit_history(
    plan: Option<&RuntimePlan>,
) -> Result<AdminAuditHistoryView, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(AdminAuditHistoryView {
            entries: Vec::new(),
            empty_text:
                "Start the checked-in Shoppr runtime before expecting persisted operator audit history."
                    .to_string(),
            backend: "unavailable".to_string(),
            location: "runtime not started".to_string(),
            entry_count: 0,
        });
    };

    let snapshot = match RuntimeWasmHostServices::new(plan.clone()).metadata_snapshot(100) {
        Ok(snapshot) => snapshot,
        Err(reason) => {
            let fallback = AdminAuditLog::open(plan);
            let fallback_entries = fallback.recent_entries(100).map_err(|error| {
                TemplateModelError::TemplateRead {
                    path: "admin-audit-log".to_string(),
                    message: format!(
                        "audit history is currently unavailable: {reason}; local fallback also failed: {error}"
                    ),
                }
            })?;
            let entries = fallback_entries
                .iter()
                .map(|record| {
                    admin_audit_entry_model(
                        record.recorded_at_unix_seconds,
                        record.actor.as_str(),
                        record.kind.as_str(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(AdminAuditHistoryView {
                empty_text: format!(
                    "Audit history is using the local fallback log because the shared metadata backend is unavailable: {reason}"
                ),
                backend: "local-fallback".to_string(),
                location: fallback.location_label(),
                entry_count: fallback_entries.len(),
                entries,
            });
        }
    };

    let entries = snapshot
        .recent_records
        .iter()
        .map(|record| {
            admin_audit_entry_model(
                record.recorded_at_unix_seconds,
                record
                    .principal_id
                    .as_deref()
                    .unwrap_or(record.principal_kind.as_str()),
                record.kind.as_str(),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AdminAuditHistoryView {
        empty_text: format!(
            "Audit backend `{}` at `{}` is live, but no privileged admin actions have been captured yet for this Shoppr runtime.",
            snapshot.backend.as_str(),
            snapshot.location
        ),
        backend: snapshot.backend.as_str().to_string(),
        location: snapshot.location,
        entry_count: snapshot.entry_count,
        entries,
    })
}

fn admin_audit_entry_model(
    recorded_at_unix_seconds: i64,
    actor: &str,
    kind: &str,
) -> Result<RenderModel, TemplateModelError> {
    let parsed = parse_operator_audit_record(kind);
    let resource = if parsed.resource_id.trim().is_empty() {
        parsed.resource_kind.clone()
    } else {
        format!("{}:{}", parsed.resource_kind, parsed.resource_id)
    };
    let detail = parsed.detail.trim().to_string();
    RenderModel::new()
        .with_value(
            "when",
            RenderValue::text(recorded_at_unix_seconds.to_string()),
        )?
        .with_value("actor", RenderValue::text(actor.to_string()))?
        .with_value(
            "action",
            RenderValue::text(admin_audit_action_label(parsed.action.as_str())),
        )?
        .with_value("capability", RenderValue::text(parsed.capability))?
        .with_value("resource", RenderValue::text(resource))?
        .with_value(
            "outcome",
            RenderValue::text(admin_audit_outcome_label(parsed.outcome.as_str())),
        )?
        .with_bool("has_detail", !detail.is_empty())?
        .with_value("detail", RenderValue::text(detail))
}

fn parse_operator_audit_record(kind: &str) -> ParsedOperatorAuditRecord {
    if !kind.contains('=') {
        return ParsedOperatorAuditRecord {
            action: kind.to_string(),
            capability: admin_audit_capability_fallback(kind).to_string(),
            outcome: "succeeded".to_string(),
            ..ParsedOperatorAuditRecord::default()
        };
    }

    let mut parsed = ParsedOperatorAuditRecord::default();
    for (key, value) in form_urlencoded::parse(kind.as_bytes()) {
        match key.as_ref() {
            "action" => parsed.action = value.into_owned(),
            "route" => parsed.route = value.into_owned(),
            "capability" => parsed.capability = value.into_owned(),
            "resource_kind" => parsed.resource_kind = value.into_owned(),
            "resource_id" => parsed.resource_id = value.into_owned(),
            "outcome" => parsed.outcome = value.into_owned(),
            "detail" => parsed.detail = value.into_owned(),
            _ => {}
        }
    }

    if parsed.capability.is_empty() {
        parsed.capability = admin_audit_capability_fallback(parsed.action.as_str()).to_string();
    }
    if parsed.outcome.is_empty() {
        parsed.outcome = "succeeded".to_string();
    }
    parsed
}

fn admin_audit_capability_fallback(action: &str) -> &'static str {
    match action {
        "cms.pages.save-draft" => "cms.page.edit",
        "cms.pages.publish" | "cms.pages.unpublish" => "cms.page.publish",
        "cms.navigation.save" => "cms.navigation.edit",
        "cms.redirects.save" => "cms.page.edit",
        "commerce.catalog-admin-update.product"
        | "commerce.catalog-admin-update.collection"
        | "commerce.catalog-admin-update" => "catalog.product.edit",
        "commerce.order-refund" => "order.refund.issue",
        "commerce.order-fulfill" => "order.refund.issue",
        _ => "",
    }
}

fn admin_audit_action_label(action: &str) -> &'static str {
    match action {
        "cms.pages.save-draft" => "Save draft",
        "cms.pages.publish" => "Publish page",
        "cms.pages.unpublish" => "Unpublish page",
        "cms.navigation.save" => "Save navigation",
        "cms.redirects.save" => "Save redirects",
        "commerce.catalog-admin-update.product" => "Update product",
        "commerce.catalog-admin-update.collection" => "Update collection",
        "commerce.order-refund" => "Issue refund",
        "commerce.order-fulfill" => "Mark fulfilled",
        _ => "Privileged action",
    }
}

fn admin_audit_outcome_label(outcome: &str) -> &'static str {
    match outcome {
        "succeeded" => "Succeeded",
        "rejected" => "Rejected",
        _ => "Recorded",
    }
}

fn admin_panels(
    locale: &str,
    fixture: &StorefrontFixture,
    order_count: usize,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let content_pages = content_pages(locale)?;
    Ok(vec![
        admin_panel(
            "Catalog",
            "Inspect products and collections",
            "/admin/catalog/products",
            &format!(
                "{} products and {} collections are currently represented in the sample app.",
                fixture.product_cards.len(),
                fixture.catalog_sections.len()
            ),
        )?,
        admin_panel(
            "Orders",
            "Review recent purchases",
            "/admin/orders",
            &format!("{order_count} completed orders are available for operator review."),
        )?,
        admin_panel(
            "Content",
            "Review live route inventory",
            "/admin/pages",
            &format!(
                "{} content routes are represented in the checked-in Shoppr sample app.",
                content_pages.len()
            ),
        )?,
    ])
}

fn admin_panel(
    title: &str,
    label: &str,
    href: &str,
    summary: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("label", RenderValue::text(label))?
        .with_value("href", RenderValue::text(href))?
        .with_value("summary", RenderValue::text(summary))
}

fn content_pages(locale: &str) -> Result<Vec<RenderModel>, TemplateModelError> {
    Ok(vec![
        content_page(
            "Home",
            "/",
            "public",
            "The landing page for the Shoppr storefront.",
        )?,
        content_page(
            "Catalog",
            &localized_shop_path(locale),
            "public",
            "The main shopping entry point for products and collections.",
        )?,
        content_page(
            "Collections",
            &localized_collections_path(locale),
            "public",
            "Curated collection landing pages for merchandising journeys.",
        )?,
        content_page(
            "Account",
            "/account",
            "account",
            "Customer account hub for orders and membership state.",
        )?,
        content_page(
            "Order history",
            "/account/orders",
            "account",
            "Order history and post-checkout confirmation records.",
        )?,
        content_page(
            "Memberships",
            "/account/memberships",
            "account",
            "Membership state and entitlement guidance for signed-in customers.",
        )?,
    ])
}

fn content_page(
    title: &str,
    href: &str,
    surface: &str,
    summary: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("href", RenderValue::text(href))?
        .with_value("surface", RenderValue::text(surface))?
        .with_value("summary", RenderValue::text(summary))
}

fn query_first<'a>(query_params: &'a RequestFieldMap, name: &str) -> Option<&'a str> {
    query_params
        .get(name)
        .and_then(|values| values.first())
        .map(String::as_str)
}

fn query_flag(query_params: &RequestFieldMap, name: &str) -> bool {
    query_first(query_params, name)
        .is_some_and(|value| matches!(value, "1" | "true" | "yes" | "on"))
}

fn cms_admin_form_targets_new_page(form_state: Option<&StorefrontFormState>) -> bool {
    let Some(form_state) = form_state else {
        return false;
    };
    let page_id = form_state
        .fields
        .get("page_id")
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    if !page_id.is_empty() {
        return false;
    }
    ["page_title", "page_slug", "page_summary", "page_body_html"]
        .iter()
        .any(|field| form_state.fields.contains_key(*field))
}

fn cms_admin_workspace(plan: &RuntimePlan) -> Result<CmsAdminWorkspace, TemplateModelError> {
    CmsAdminWorkspace::load(plan).map_err(|message| TemplateModelError::TemplateRead {
        path: "cms-admin-workspace".to_string(),
        message,
    })
}

fn default_cms_admin_workspace() -> CmsAdminWorkspace {
    crate::default_workspace()
}

fn cms_admin_pages_model(
    workspace: &CmsAdminWorkspace,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    workspace
        .pages
        .iter()
        .map(|page| {
            RenderModel::new()
                .with_value("id", RenderValue::text(page.id.clone()))?
                .with_value("title", RenderValue::text(page.draft.title.clone()))?
                .with_value("slug", RenderValue::text(page.draft.slug.clone()))?
                .with_value(
                    "status_label",
                    RenderValue::text(page.status_label().to_string()),
                )?
                .with_value("summary", RenderValue::text(page.draft.summary.clone()))?
                .with_value(
                    "edit_href",
                    RenderValue::text(format!("/admin/pages?page={}", page.id)),
                )?
                .with_bool("has_live_path", page.live_path().is_some())?
                .with_value(
                    "live_path",
                    RenderValue::text(page.live_path().unwrap_or_default()),
                )
        })
        .collect()
}

fn cms_admin_selected_page_model(page: CmsAdminPage) -> Result<RenderModel, TemplateModelError> {
    let preview_html = TrustedHtml::new(page.draft.body_html.clone())?;
    RenderModel::new()
        .with_value("id", RenderValue::text(page.id.clone()))?
        .with_value("title", RenderValue::text(page.draft.title.clone()))?
        .with_value("slug", RenderValue::text(page.draft.slug.clone()))?
        .with_value("summary", RenderValue::text(page.draft.summary.clone()))?
        .with_value(
            "body_source",
            RenderValue::text(page.draft.body_html.clone()),
        )?
        .with_value("body_html", RenderValue::trusted_html(preview_html))?
        .with_value(
            "status_label",
            RenderValue::text(page.status_label().to_string()),
        )?
        .with_bool("has_live_path", page.live_path().is_some())?
        .with_value(
            "live_path",
            RenderValue::text(page.live_path().unwrap_or_default()),
        )?
        .with_bool("has_preview_path", true)?
        .with_value("preview_path", RenderValue::text(page.preview_path()))
}

fn cms_admin_selected_page_model_with_form_state(
    page: Option<CmsAdminPage>,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let page_id = form_state
        .and_then(|state| state.fields.get("page_id"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.id.clone()))
        .unwrap_or_default();
    let title = form_state
        .and_then(|state| state.fields.get("page_title"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.title.clone()))
        .unwrap_or_default();
    let slug = form_state
        .and_then(|state| state.fields.get("page_slug"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.slug.clone()))
        .unwrap_or_default();
    let summary = form_state
        .and_then(|state| state.fields.get("page_summary"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.summary.clone()))
        .unwrap_or_default();
    let body_html = form_state
        .and_then(|state| state.fields.get("page_body_html"))
        .cloned()
        .or_else(|| page.as_ref().map(|page| page.draft.body_html.clone()))
        .unwrap_or_else(|| "<p>Create a draft page to preview it.</p>".to_string());
    let status_label = page
        .as_ref()
        .map(|page| page.status_label().to_string())
        .unwrap_or_else(|| "Draft only".to_string());
    let live_path = page
        .as_ref()
        .and_then(|page| page.live_path())
        .unwrap_or_default();
    let has_live_path = !live_path.is_empty();
    let preview_path = page
        .as_ref()
        .map(|page| page.preview_path())
        .or_else(|| (!page_id.is_empty()).then(|| format!("/admin/pages/preview?page={page_id}")))
        .unwrap_or_default();
    RenderModel::new()
        .with_value("id", RenderValue::text(page_id))?
        .with_value("title", RenderValue::text(title))?
        .with_value("slug", RenderValue::text(slug.clone()))?
        .with_value("summary", RenderValue::text(summary))?
        .with_value("body_source", RenderValue::text(body_html.clone()))?
        .with_value(
            "body_html",
            RenderValue::trusted_html(TrustedHtml::new(body_html)?),
        )?
        .with_value("status_label", RenderValue::text(status_label))?
        .with_bool("has_live_path", has_live_path)?
        .with_value("live_path", RenderValue::text(live_path))?
        .with_bool("has_preview_path", !preview_path.is_empty())?
        .with_value("preview_path", RenderValue::text(preview_path))
}

fn empty_cms_admin_selected_page_model() -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("id", RenderValue::text(String::new()))?
        .with_value("title", RenderValue::text(String::new()))?
        .with_value("slug", RenderValue::text(String::new()))?
        .with_value("summary", RenderValue::text(String::new()))?
        .with_value("body_source", RenderValue::text(String::new()))?
        .with_value(
            "body_html",
            RenderValue::trusted_html(TrustedHtml::new(
                "<p>Create a draft page to preview it.</p>",
            )?),
        )?
        .with_value("status_label", RenderValue::text("Draft only"))?
        .with_bool("has_live_path", false)?
        .with_value("live_path", RenderValue::text(String::new()))?
        .with_bool("has_preview_path", false)?
        .with_value("preview_path", RenderValue::text(String::new()))
}

fn cms_navigation_items_model(
    items: &[CmsAdminNavigationItem],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            RenderModel::new()
                .with_value("label", RenderValue::text(item.label.clone()))?
                .with_value("href", RenderValue::text(item.href.clone()))?
                .with_value(
                    "label_field",
                    RenderValue::text(format!("nav_label_{index}")),
                )?
                .with_value("href_field", RenderValue::text(format!("nav_href_{index}")))
        })
        .collect()
}

fn cms_redirects_model(
    redirects: &[CmsAdminRedirect],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    redirects
        .iter()
        .enumerate()
        .map(|(index, redirect)| {
            RenderModel::new()
                .with_value("from", RenderValue::text(redirect.from.clone()))?
                .with_value("to", RenderValue::text(redirect.to.clone()))?
                .with_bool("permanent", redirect.permanent)?
                .with_value(
                    "from_field",
                    RenderValue::text(format!("redirect_from_{index}")),
                )?
                .with_value("to_field", RenderValue::text(format!("redirect_to_{index}")))?
                .with_value(
                    "permanent_field",
                    RenderValue::text(format!("redirect_permanent_{index}")),
                )
        })
        .collect()
}

fn cms_live_page_model(
    workspace: &CmsAdminWorkspace,
    slug: &str,
) -> Result<RenderModel, TemplateModelError> {
    if let Some(page) = workspace.live_page_by_slug(slug) {
        let live = page
            .live
            .as_ref()
            .expect("live page should have a live revision");
        return RenderModel::new()
            .with_bool("is_published", true)?
            .with_value("title", RenderValue::text(live.title.clone()))?
            .with_value("summary", RenderValue::text(live.summary.clone()))?
            .with_value(
                "body_html",
                RenderValue::trusted_html(TrustedHtml::new(live.body_html.clone())?),
            )?
            .with_value("slug", RenderValue::text(live.slug.clone()));
    }

    RenderModel::new()
        .with_bool("is_published", false)?
        .with_value("title", RenderValue::text("Page unavailable"))?
        .with_value(
            "summary",
            RenderValue::text(
                "This CMS page is not published yet. Use the Shoppr admin workflow to publish it before linking customers to this path.",
            ),
        )?
        .with_value(
            "body_html",
            RenderValue::trusted_html(TrustedHtml::new(
                "<p>The requested CMS page is not live yet.</p>",
            )?),
        )?
        .with_value("slug", RenderValue::text(slug.to_string()))
}

fn merge_cms_page_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    model
        .with_bool("has_errors", has_errors)?
        .with_value(
            "error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Update the page draft and save again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_cms_navigation_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    model
        .with_bool("has_errors", has_errors)?
        .with_value(
            "error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Update the navigation items and save again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_cms_redirect_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    model
        .with_bool("has_errors", has_errors)?
        .with_value(
            "error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Update the redirect rules and save again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_order_refund_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    model
        .with_bool("has_refund_errors", !errors.is_empty())?
        .with_value(
            "refund_error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| {
                        "Review the refund request before issuing another order change.".to_string()
                    }),
            ),
        )?
        .with_list("refund_errors", errors)
}

fn cart_item_from_storefront(
    catalog: &StorefrontCatalog,
    locale: &str,
    line: &StorefrontCartLine,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let quantity_field = format!("quantity_{}", line.sku);
    let quantity_value = form_state
        .and_then(|state| state.fields.get(&quantity_field))
        .cloned()
        .unwrap_or_else(|| line.quantity.to_string());
    let quantity_error = form_state.and_then(|state| state.field_errors.get(&quantity_field));
    let model = cart_item(
        &line.title,
        &line.variant_title,
        &quantity_value,
        &line.total,
    )?
    .with_value("quantity_field", RenderValue::text(quantity_field))
    .and_then(|model| model.with_bool("has_quantity_error", quantity_error.is_some()))
    .and_then(|model| {
        model.with_value(
            "quantity_error",
            RenderValue::text(quantity_error.cloned().unwrap_or_default()),
        )
    })?;

    decorate_cart_item_with_catalog_context(model, catalog, locale, &line.sku, &line.title)
}

fn decorate_cart_item_with_catalog_context(
    model: RenderModel,
    catalog: &StorefrontCatalog,
    locale: &str,
    sku_or_handle: &str,
    title: &str,
) -> Result<RenderModel, TemplateModelError> {
    let Some(product) = catalog_product_for_cart_item(catalog, sku_or_handle, title) else {
        return model
            .with_bool("has_product_link", false)?
            .with_value("product_url", RenderValue::text(String::new()))?
            .with_value("collection_url", RenderValue::text(String::new()))?
            .with_value("collection_name", RenderValue::text(String::new()));
    };

    let collection_name = catalog
        .collection(&product.collection_handle)
        .map(|collection| collection.title.as_str())
        .unwrap_or("Collection");

    model
        .with_bool("has_product_link", true)?
        .with_value(
            "product_url",
            RenderValue::text(localized_product_path(locale, &product.handle)),
        )?
        .with_value(
            "collection_url",
            RenderValue::text(localized_collection_path(
                locale,
                &product.collection_handle,
            )),
        )?
        .with_value("collection_name", RenderValue::text(collection_name))
}

fn catalog_product_for_cart_item<'a>(
    catalog: &'a StorefrontCatalog,
    sku_or_handle: &str,
    title: &str,
) -> Option<&'a StorefrontProductDefinition> {
    catalog.product_by_sku_or_handle(sku_or_handle).or_else(|| {
        catalog
            .products
            .iter()
            .find(|product| product.title == title)
    })
}

fn checkout_customer(
    principal: Option<&PrincipalContext>,
) -> Result<RenderModel, TemplateModelError> {
    let email = principal
        .and_then(|principal| principal.principal_id.clone())
        .filter(|candidate| looks_like_email(candidate))
        .unwrap_or_default();
    let display_name = principal
        .and_then(|principal| principal.principal_id.as_deref())
        .map(display_name_from_principal_id)
        .unwrap_or_else(|| "Guest Checkout".to_string());
    RenderModel::new()
        .with_value("display_name", RenderValue::text(display_name))?
        .with_value("email", RenderValue::text(email))
}

fn checkout_form_from_storefront(
    plan: Option<&RuntimePlan>,
    payment: &StorefrontPaymentSnapshot,
    principal: Option<&PrincipalContext>,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let provider_code = payment_provider_code(plan);
    let provider_label = payment_provider_label(plan);
    let provider_summary = payment_provider_summary(plan);
    let submit_label = payment_submit_label(plan);
    let payment_method = form_state
        .and_then(|state| state.fields.get("payment_method"))
        .cloned()
        .filter(|value| !value.is_empty())
        .or_else(|| payment.method.clone())
        .unwrap_or_else(|| "card".to_string());
    let checkout_email = form_state
        .and_then(|state| state.fields.get("checkout_email"))
        .cloned()
        .or_else(|| {
            payment.checkout_email.clone().or_else(|| {
                principal
                    .and_then(|principal| principal.principal_id.clone())
                    .filter(|candidate| looks_like_email(candidate))
            })
        })
        .unwrap_or_default();
    let payment_reference = payment
        .reference
        .clone()
        .unwrap_or_else(|| "PAYMENT-PENDING".to_string());
    let checkout_intent = form_state
        .and_then(|state| state.fields.get("checkout_intent"))
        .cloned()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| payment_reference.clone());
    let payment_last4 = form_state
        .and_then(|state| state.fields.get("payment_last4"))
        .cloned()
        .or_else(|| payment.last4.clone())
        .unwrap_or_default();
    let delivery_name = form_state
        .and_then(|state| state.fields.get("delivery_name"))
        .cloned()
        .unwrap_or_default();
    let delivery_note = form_state
        .and_then(|state| state.fields.get("delivery_note"))
        .cloned()
        .unwrap_or_default();
    let terms_accepted = form_state
        .and_then(|state| state.fields.get("terms_accepted"))
        .is_some();
    let has_checkout_email = !checkout_email.is_empty();
    let model = RenderModel::new()
        .with_value("payment_reference", RenderValue::text(payment_reference))?
        .with_value("payment_method", RenderValue::text(payment_method.clone()))?
        .with_value("checkout_email", RenderValue::text(checkout_email))?
        .with_bool("has_checkout_email", has_checkout_email)?
        .with_value("payment_last4", RenderValue::text(payment_last4))?
        .with_value("checkout_intent", RenderValue::text(checkout_intent))?
        .with_value("delivery_name", RenderValue::text(delivery_name))?
        .with_value("delivery_note", RenderValue::text(delivery_note))?
        .with_bool("terms_accepted", terms_accepted)?
        .with_value(
            "payment_method_label",
            RenderValue::text(payment_method_label(Some(payment_method.as_str()))),
        )?
        .with_value("payment_status", RenderValue::text(payment.status.clone()))?
        .with_value(
            "payment_status_label",
            RenderValue::text(payment_status_label(&payment.status)),
        )?
        .with_value("provider_code", RenderValue::text(provider_code.to_string()))?
        .with_value(
            "provider_label",
            RenderValue::text(provider_label.to_string()),
        )?
        .with_value("provider_summary", RenderValue::text(provider_summary))?
        .with_value("submit_label", RenderValue::text(submit_label))?
        .with_bool("has_payment_reference", payment.reference.is_some())?
        .with_bool("has_payment_last4", payment.last4.is_some())?;
    merge_checkout_form_feedback(model, form_state)
}

fn payment_provider_code(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.code.clone())
        .unwrap_or_else(|| "platform_fallback".to_string())
}

fn payment_provider_label(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.label())
        .unwrap_or_else(|| "Platform fallback payment path".to_string())
}

fn payment_provider_summary(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.summary())
        .unwrap_or_else(|| {
            "This checkout is using the platform fallback payment path until a provider-backed handoff is installed.".to_string()
        })
}

fn payment_submit_label(plan: Option<&RuntimePlan>) -> String {
    configured_payment_provider(plan)
        .map(|provider| provider.submit_label())
        .unwrap_or_else(|| "Place order".to_string())
}

fn configured_payment_provider(
    plan: Option<&RuntimePlan>,
) -> Option<crate::server::CommercePaymentProviderConfig> {
    plan.and_then(|plan| crate::server::configured_commerce_payment_provider(&plan.config))
}

fn payment_method_label(method: Option<&str>) -> String {
    match method.unwrap_or_default() {
        "card" => "Card".to_string(),
        "gift_credit" => "Gift credit".to_string(),
        "manual" => "Manual capture".to_string(),
        "" => "Payment method pending".to_string(),
        other => display_status_label(other),
    }
}

fn payment_status_label(status: &str) -> String {
    match status {
        "not_started" => "Not started".to_string(),
        "ready_for_payment" => "Ready for payment".to_string(),
        "provider_pending" => "Awaiting provider confirmation".to_string(),
        "captured" => "Captured".to_string(),
        "authorized" => "Authorized".to_string(),
        "failed" => "Failed".to_string(),
        "refunded" => "Refunded".to_string(),
        other => display_status_label(other),
    }
}

fn payment_summary(method: Option<&str>, last4: Option<&str>, reference: Option<&str>) -> String {
    let method = payment_method_label(method);
    match (last4, reference) {
        (Some(last4), Some(reference)) => format!("{method} ending {last4}, reference {reference}"),
        (Some(last4), None) => format!("{method} ending {last4}"),
        (None, Some(reference)) => format!("{method}, reference {reference}"),
        (None, None) => method,
    }
}

fn template_store_error(error: crate::storefront::StorefrontStateError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "storefront-state".to_string(),
        message: error.to_string(),
    }
}

fn template_membership_error(error: MembershipModelError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "membership-projection".to_string(),
        message: error.to_string(),
    }
}

fn template_commerce_error(error: coil_commerce::CommerceModelError) -> TemplateModelError {
    TemplateModelError::TemplateRead {
        path: "membership-projection".to_string(),
        message: error.to_string(),
    }
}

fn confirmation_line_items_from_storefront(
    order: &StorefrontOrderSnapshot,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    order
        .lines
        .iter()
        .map(|line| {
            RenderModel::new()
                .with_value("title", RenderValue::text(line.title.clone()))?
                .with_value("quantity", RenderValue::text(line.quantity.to_string()))?
                .with_value("total", RenderValue::text(line.total.clone()))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn title_case_handle(handle: &str) -> String {
    handle
        .split('-')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut word = first.to_uppercase().collect::<String>();
                    word.push_str(chars.as_str());
                    word
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn localized_shop_path(locale: &str) -> String {
    format!("/{}/shop", locale.trim_matches('/'))
}

fn localized_collections_path(locale: &str) -> String {
    format!("/{}/shop/collections", locale.trim_matches('/'))
}

fn localized_collection_path(locale: &str, slug: &str) -> String {
    format!(
        "/{}/shop/collections/{}",
        locale.trim_matches('/'),
        slug.trim_matches('/')
    )
}

fn localized_product_path(locale: &str, slug: &str) -> String {
    format!(
        "/{}/shop/products/{}",
        locale.trim_matches('/'),
        slug.trim_matches('/')
    )
}

#[derive(Clone)]
struct AccountSurfaceBindings {
    account: RenderModel,
    customer: RenderModel,
    recent_orders: Vec<RenderModel>,
    membership_summary: RenderModel,
}

fn flash_messages_model(messages: &[FlashMessage]) -> Result<Vec<RenderModel>, TemplateModelError> {
    messages
        .iter()
        .map(|message| {
            RenderModel::new()
                .with_value(
                    "level",
                    RenderValue::text(format!("{:?}", message.level).to_ascii_lowercase()),
                )?
                .with_value("text", RenderValue::text(message.text.clone()))
        })
        .collect::<Result<Vec<_>, _>>()
}

fn account_surface_bindings(
    plan: Option<&RuntimePlan>,
    fixture: &StorefrontFixture,
    locale: &str,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
    include_pending_membership: bool,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let Some(session) = session else {
        return fixture_account_surface_bindings(fixture, locale);
    };
    if session.session_id.is_none() {
        return fixture_account_surface_bindings(fixture, locale);
    }

    live_account_surface_bindings(plan, locale, session, principal, include_pending_membership)
}

fn fixture_account_surface_bindings(
    fixture: &StorefrontFixture,
    locale: &str,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let latest_preview_order = sample_completed_order();
    let has_recent_orders = !fixture.recent_orders.is_empty();
    let orders_cta_url = if has_recent_orders {
        "/account/orders".to_string()
    } else {
        localized_shop_path(locale)
    };
    let orders_cta_label = if has_recent_orders {
        "View order history"
    } else {
        "Browse storefront"
    };
    Ok(AccountSurfaceBindings {
        account: RenderModel::new()
            .with_bool("has_live_session", false)?
            .with_bool("has_principal", false)?
            .with_bool("has_customer_email", true)?
            .with_bool("has_recent_orders", has_recent_orders)?
            .with_bool("has_membership", true)?
            .with_bool("has_latest_order", true)?
            .with_bool("has_pending_membership_order", false)?
            .with_bool("needs_membership_purchase", false)?
            .with_value("state_source", RenderValue::text("fixture-preview"))?
            .with_value(
                "state_summary",
                RenderValue::text(
                    "Previewing deterministic account content until a live storefront session is resolved.",
                ),
            )?
            .with_value(
                "orders_empty_text",
                RenderValue::text(
                    "Recent orders will appear here once the customer has completed checkout.",
                ),
            )?
            .with_value(
                "membership_empty_text",
                RenderValue::text(
                    "No membership is attached yet. Join to unlock early-access drops and concierge support.",
                ),
            )?
            .with_value("orders_cta_url", RenderValue::text(orders_cta_url))?
            .with_value("orders_cta_label", RenderValue::text(orders_cta_label))?
            .with_value(
                "membership_cta_url",
                RenderValue::text(localized_collection_path(locale, "memberships")),
            )?
            .with_value(
                "latest_order_reference",
                RenderValue::text(latest_preview_order.id.to_string()),
            )?
            .with_value(
                "latest_order_status",
                RenderValue::text(latest_preview_order.history_status_label()),
            )?,
        customer: fixture.customer.clone(),
        recent_orders: fixture.recent_orders.clone(),
        membership_summary: fixture.membership_summary.clone(),
    })
}

fn live_account_surface_bindings(
    plan: Option<&RuntimePlan>,
    locale: &str,
    session: &SessionContext,
    principal: Option<&PrincipalContext>,
    include_pending_membership: bool,
) -> Result<AccountSurfaceBindings, TemplateModelError> {
    let snapshot = live_storefront_state(plan, Some(session), principal)?;
    let principal_id = principal.and_then(|principal| principal.principal_id.as_deref());
    let recent_orders = recent_orders_from_storefront(snapshot.as_ref())?;
    let has_recent_orders = !recent_orders.is_empty();
    let latest_order = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.recent_orders.first().cloned());
    let email = principal_id
        .filter(|candidate| looks_like_email(candidate))
        .map(str::to_string)
        .or_else(|| {
            latest_order
                .as_ref()
                .and_then(|order| order.payment.checkout_email.clone())
        })
        .unwrap_or_default();
    let display_name = principal_id
        .map(display_name_from_principal_id)
        .or_else(|| {
            if email.is_empty() {
                None
            } else {
                Some(display_name_from_principal_id(&email))
            }
        })
        .unwrap_or_else(|| "Current Browser Session".to_string());
    let membership_summary =
        membership_summary_from_storefront(snapshot.as_ref(), include_pending_membership)?;
    let has_membership = membership_summary.is_some();
    let has_latest_order = latest_order.is_some();
    let has_pending_membership_order = !has_membership && has_latest_order;
    let needs_membership_purchase = !has_membership && !has_latest_order;
    let latest_order_reference = latest_order
        .as_ref()
        .map(|order| order.order_id.clone())
        .unwrap_or_default();
    let latest_order_status = latest_order
        .as_ref()
        .map(|order| display_status_label(&order.status))
        .unwrap_or_default();
    let state_summary = account_state_summary(
        if principal_id.is_some() {
            "Using the live storefront session identity for this account view. Order history and membership state render from the current signed-in browser session."
        } else {
            "This account area follows the current browser session. Completed checkouts from this browser become the order history shown here, and any qualifying membership purchase from this browser appears here after payment capture."
        },
        latest_order.as_ref(),
    );
    let orders_cta_url = if has_recent_orders {
        "/account/orders".to_string()
    } else {
        localized_shop_path(locale)
    };
    let orders_cta_label = if has_recent_orders {
        "View order history"
    } else {
        "Browse storefront"
    };

    Ok(AccountSurfaceBindings {
        account: RenderModel::new()
            .with_bool("has_live_session", session.session_id.is_some())?
            .with_bool("has_principal", principal_id.is_some())?
            .with_bool("has_customer_email", !email.is_empty())?
            .with_bool("has_recent_orders", has_recent_orders)?
            .with_bool("has_membership", has_membership)?
            .with_bool("has_latest_order", has_latest_order)?
            .with_bool("has_pending_membership_order", has_pending_membership_order)?
            .with_bool("needs_membership_purchase", needs_membership_purchase)?
            .with_value("state_source", RenderValue::text("storefront-session"))?
            .with_value("state_summary", RenderValue::text(state_summary))?
            .with_value(
                "orders_empty_text",
                RenderValue::text(
                    if principal_id.is_some() {
                        "No order history is attached to this signed-in account yet. Completed storefront purchases will appear here once live account history is available."
                    } else {
                        "This browser session has no completed orders yet. Orders placed from this browser will appear here automatically after checkout."
                    },
                ),
            )?
            .with_value(
                "membership_empty_text",
                RenderValue::text(
                    if principal_id.is_some() {
                        "No active membership is attached to this signed-in account yet. Join from the storefront to unlock early access and renewal visibility."
                    } else {
                        "No active membership is attached to this browser session yet. A qualifying membership purchase completed here will appear after payment capture."
                    },
                ),
            )?
            .with_value("orders_cta_url", RenderValue::text(orders_cta_url))?
            .with_value("orders_cta_label", RenderValue::text(orders_cta_label))?
            .with_value(
                "membership_cta_url",
                RenderValue::text(localized_collection_path(locale, "memberships")),
            )?
            .with_value(
                "latest_order_reference",
                RenderValue::text(latest_order_reference),
            )?
            .with_value("latest_order_status", RenderValue::text(latest_order_status))?,
        customer: RenderModel::new()
            .with_value("display_name", RenderValue::text(display_name))?
            .with_value("email", RenderValue::text(email))?,
        recent_orders,
        membership_summary: membership_summary.unwrap_or(empty_membership_summary()?),
    })
}

fn recent_orders_from_storefront(
    snapshot: Option<&StorefrontStateSnapshot>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(snapshot) = snapshot else {
        return Ok(Vec::new());
    };

    snapshot
        .recent_orders
        .iter()
        .map(account_order_from_storefront)
        .collect::<Result<Vec<_>, _>>()
}

fn membership_summary_from_storefront(
    snapshot: Option<&StorefrontStateSnapshot>,
    include_pending_membership: bool,
) -> Result<Option<RenderModel>, TemplateModelError> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };

    let Some(projected) = projected_membership_state(snapshot, include_pending_membership)? else {
        return Ok(None);
    };

    membership_summary(
        &projected.tier_name,
        projected.status_label(),
        &projected.renewal_text,
    )
    .map(Some)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectedMembershipState {
    tier_name: String,
    status: SubscriptionStatus,
    renewal_text: String,
}

impl ProjectedMembershipState {
    fn status_label(&self) -> &'static str {
        match self.status {
            SubscriptionStatus::PendingActivation => "Pending activation",
            SubscriptionStatus::Active => "Active",
            SubscriptionStatus::InGracePeriod => "In grace period",
            SubscriptionStatus::Paused => "Paused",
            SubscriptionStatus::Cancelled => "Cancelled",
            SubscriptionStatus::Expired => "Expired",
        }
    }
}

fn projected_membership_state(
    snapshot: &StorefrontStateSnapshot,
    include_pending_membership: bool,
) -> Result<Option<ProjectedMembershipState>, TemplateModelError> {
    if let Some(active) = projected_membership_state_for_statuses(snapshot, &["paid", "fulfilled"])?
    {
        return Ok(Some(active));
    }

    if include_pending_membership {
        return projected_membership_state_for_statuses(snapshot, &["pending_payment"]);
    }

    Ok(None)
}

fn projected_membership_state_for_statuses(
    snapshot: &StorefrontStateSnapshot,
    eligible_statuses: &[&str],
) -> Result<Option<ProjectedMembershipState>, TemplateModelError> {
    for order in &snapshot.recent_orders {
        if !eligible_statuses
            .iter()
            .any(|status| order.status.as_str() == *status)
        {
            continue;
        }
        if let Some(projected) = projected_membership_state_for_order(snapshot, order)? {
            return Ok(Some(projected));
        }
    }
    Ok(None)
}

fn projected_membership_state_for_order(
    snapshot: &StorefrontStateSnapshot,
    order: &StorefrontOrderSnapshot,
) -> Result<Option<ProjectedMembershipState>, TemplateModelError> {
    let outcomes = storefront_membership_outcomes(order)?;
    if outcomes.is_empty() {
        return Ok(None);
    }

    let order_id = OrderId::new(order.order_id.clone()).map_err(template_commerce_error)?;
    let member_id = storefront_member_account_id(snapshot)?;
    let starts_at = MembershipInstant::from_unix_seconds(order.created_at_unix_seconds);
    let catalog = storefront_membership_catalog(order)?;
    let mut provisioned = catalog
        .provision_from_order_outcomes(order_id, member_id, &outcomes, starts_at)
        .map_err(template_membership_error)?;
    let Some(mut provisioned) = provisioned.drain(..).next() else {
        return Ok(None);
    };

    if matches!(order.status.as_str(), "paid" | "fulfilled") {
        provisioned
            .subscription
            .activate(starts_at)
            .map_err(template_membership_error)?;
    }

    let tier_name = catalog
        .tier(&provisioned.subscription.tier_id)
        .map(|tier| tier.title.clone())
        .unwrap_or_else(|| {
            order
                .lines
                .iter()
                .find(|line| line.product_kind == "membership")
                .map(|line| line.title.clone())
                .unwrap_or_else(|| "Membership".to_string())
        });
    let renewal_text = match provisioned.subscription.status {
        SubscriptionStatus::PendingActivation => format!(
            "Included with order {}. Membership access will activate automatically after payment capture for this order.",
            order.order_id
        ),
        SubscriptionStatus::Active => format!(
            "Activated from order {}. Membership access is live for this account view.",
            order.order_id
        ),
        SubscriptionStatus::InGracePeriod => format!(
            "Membership from order {} is in its grace period while renewal is resolved.",
            order.order_id
        ),
        SubscriptionStatus::Paused => format!(
            "Membership from order {} is currently paused.",
            order.order_id
        ),
        SubscriptionStatus::Cancelled => format!(
            "Membership from order {} has been cancelled.",
            order.order_id
        ),
        SubscriptionStatus::Expired => {
            format!("Membership from order {} has expired.", order.order_id)
        }
    };

    Ok(Some(ProjectedMembershipState {
        tier_name,
        status: provisioned.subscription.status,
        renewal_text,
    }))
}

fn storefront_membership_catalog(
    order: &StorefrontOrderSnapshot,
) -> Result<MembershipCatalog, TemplateModelError> {
    let mut catalog = MembershipCatalog::new();
    for line in &order.lines {
        if line.product_kind != "membership" {
            continue;
        }
        let Some(entitlement_key) = line.entitlement_key.as_deref() else {
            continue;
        };
        let tier = MembershipTier::new(
            MembershipTierId::new(format!(
                "tier-{}",
                sanitize_membership_token(entitlement_key)
            ))
            .map_err(template_membership_error)?,
            line.title.clone(),
            EntitlementKey::new(entitlement_key.to_string()).map_err(template_commerce_error)?,
            10,
            infer_membership_interval(&line.variant_title),
            0,
            TierVisibility::Public,
            Vec::new(),
        )
        .map_err(template_membership_error)?;
        if catalog
            .tier_for_entitlement(&tier.entitlement_key)
            .is_none()
        {
            catalog
                .register_tier(tier)
                .map_err(template_membership_error)?;
        }
    }
    Ok(catalog)
}

fn storefront_membership_outcomes(
    order: &StorefrontOrderSnapshot,
) -> Result<Vec<coil_commerce::OrderOutcome>, TemplateModelError> {
    order
        .lines
        .iter()
        .filter(|line| line.product_kind == "membership")
        .filter_map(|line| {
            line.entitlement_key.as_deref().map(|entitlement_key| {
                EntitlementKey::new(entitlement_key.to_string())
                    .map(
                        |entitlement_key| coil_commerce::OrderOutcome::GrantMembership {
                            entitlement_key,
                            quantity: line.quantity,
                        },
                    )
                    .map_err(template_commerce_error)
            })
        })
        .collect()
}

fn storefront_member_account_id(
    snapshot: &StorefrontStateSnapshot,
) -> Result<MemberAccountId, TemplateModelError> {
    let raw = snapshot
        .principal_id
        .clone()
        .unwrap_or_else(|| format!("session-{}", snapshot.session_id));
    MemberAccountId::new(sanitize_membership_token(&raw)).map_err(template_membership_error)
}

fn sanitize_membership_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn infer_membership_interval(variant_title: &str) -> BillingInterval {
    let normalized = variant_title.to_ascii_lowercase();
    if normalized.contains("month") {
        BillingInterval::Monthly
    } else if normalized.contains("quarter") {
        BillingInterval::Quarterly
    } else {
        BillingInterval::Annual
    }
}

fn account_state_summary(base: &str, latest_order: Option<&StorefrontOrderSnapshot>) -> String {
    match latest_order {
        Some(order) => format!(
            "{base} Latest order {} is currently {}.",
            order.order_id,
            display_status_label(&order.status)
        ),
        None => base.to_string(),
    }
}

fn display_status_label(status: &str) -> String {
    status
        .split(|ch: char| matches!(ch, '-' | '_' | ' '))
        .filter(|segment| !segment.is_empty())
        .map(capitalize_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn looks_like_email(candidate: &str) -> bool {
    matches!(candidate.split_once('@'), Some((local, domain)) if !local.is_empty() && !domain.is_empty())
}

fn display_name_from_principal_id(principal_id: &str) -> String {
    let base = principal_id
        .split_once('@')
        .map(|(local, _)| local)
        .unwrap_or(principal_id);
    let words = base
        .split(|ch: char| matches!(ch, '-' | '_' | '.' | '+' | '/'))
        .filter(|segment| !segment.is_empty())
        .map(capitalize_token)
        .collect::<Vec<_>>();
    if words.is_empty() {
        "Member Account".to_string()
    } else {
        words.join(" ")
    }
}

fn capitalize_token(segment: &str) -> String {
    let mut chars = segment.chars();
    match chars.next() {
        Some(first) => {
            let mut word = first.to_uppercase().collect::<String>();
            word.push_str(chars.as_str());
            word
        }
        None => String::new(),
    }
}

#[derive(Clone)]
struct StorefrontFixture {
    catalog_sections: Vec<RenderModel>,
    product_cards: Vec<RenderModel>,
    product_cards_by_collection: BTreeMap<String, Vec<RenderModel>>,
    product_collection_handles: BTreeMap<String, String>,
    cart_items: Vec<RenderModel>,
    cart_summary: RenderModel,
    checkout: RenderModel,
    confirmation: RenderModel,
    customer: RenderModel,
    recent_orders: Vec<RenderModel>,
    membership_summary: RenderModel,
    collections: BTreeMap<String, RenderModel>,
    products: BTreeMap<String, RenderModel>,
}

impl StorefrontFixture {
    fn collection_for(&self, handle: &str) -> RenderModel {
        self.collections
            .get(handle)
            .cloned()
            .unwrap_or_else(|| self.collections["featured"].clone())
    }

    fn product_for(&self, handle: &str) -> RenderModel {
        self.products
            .get(handle)
            .cloned()
            .unwrap_or_else(|| self.products["harbor-cap"].clone())
    }

    fn product_cards_for_collection(&self, handle: &str) -> Vec<RenderModel> {
        if handle == "featured" {
            return self.product_cards.clone();
        }

        self.product_cards_by_collection
            .get(handle)
            .cloned()
            .unwrap_or_default()
    }

    fn related_product_cards_for_product(&self, handle: &str) -> Vec<RenderModel> {
        let collection_handle = self
            .product_collection_handles
            .get(handle)
            .map(String::as_str)
            .unwrap_or("featured");
        self.product_cards_for_collection(collection_handle)
    }
}

fn product_cards_by_collection(
    locale: &str,
    products: &[ProductFixture],
    plan: Option<&RuntimePlan>,
) -> Result<BTreeMap<String, Vec<RenderModel>>, TemplateModelError> {
    let mut grouped: BTreeMap<String, Vec<RenderModel>> = BTreeMap::new();
    for product in products {
        grouped
            .entry(product.collection_handle.to_string())
            .or_default()
            .push(product_model_for_locale(locale, product, plan)?);
    }
    Ok(grouped)
}

fn storefront_fixture(
    locale: &str,
    site_id: Option<&str>,
    catalog: &StorefrontCatalog,
    plan: Option<&RuntimePlan>,
) -> Result<StorefrontFixture, TemplateModelError> {
    let products_data = catalog
        .products
        .iter()
        .filter(|product| {
            catalog
                .visible_product_for_site(site_id, product.handle.as_str())
                .is_some()
        })
        .map(|product| ProductFixture {
            handle: product.handle.clone(),
            title: product.title.clone(),
            summary: product.summary.clone(),
            price: money_display_minor(product.price_minor, &product.currency),
            collection_handle: product.collection_handle.clone(),
            collection_name: catalog
                .collection(&product.collection_handle)
                .map(|collection| collection.title.clone())
                .unwrap_or_else(|| "Collection".to_string()),
        })
        .collect::<Vec<_>>();
    let product_cards = products_data
        .iter()
        .map(|product| product_model_for_locale(locale, product, plan))
        .collect::<Result<Vec<_>, _>>()?;
    let product_cards_by_collection = product_cards_by_collection(locale, &products_data, plan)?;
    let product_collection_handles = products_data
        .iter()
        .map(|product| {
            (
                product.handle.to_string(),
                product.collection_handle.to_string(),
            )
        })
        .collect();

    let collections_data = catalog
        .collections
        .iter()
        .filter(|collection| {
            catalog
                .visible_collection_for_site(site_id, collection.handle.as_str())
                .is_some()
        })
        .map(|collection| CollectionFixture {
            handle: collection.handle.clone(),
            title: collection.title.clone(),
            href: localized_collection_path(locale, &collection.handle),
            summary: collection.summary.clone(),
            label: collection.label.clone(),
        })
        .collect::<Vec<_>>();
    let catalog_sections = collections_data
        .iter()
        .map(|collection| collection_section_model(locale, collection))
        .collect::<Result<Vec<_>, _>>()?;
    let collections = collections_data
        .iter()
        .map(|collection| collection_detail_model(locale, collection, &products_data))
        .collect::<Result<Vec<_>, _>>()?;

    let current_order = sample_completed_order();
    let previous_order = sample_previous_order();

    let cart_items = current_order
        .lines
        .iter()
        .map(|line| cart_item_from_line(catalog, locale, line))
        .collect::<Result<Vec<_>, _>>()?;
    let cart_summary = RenderModel::new()
        .with_value(
            "subtotal",
            RenderValue::text(money_display(&current_order.totals.subtotal)),
        )?
        .with_value("shipping", RenderValue::text("£0.00"))?
        .with_value(
            "total",
            RenderValue::text(money_display(&current_order.totals.total)),
        )?;
    let checkout = RenderModel::new()
        .with_value("payment_reference", RenderValue::text("card-on-file"))?
        .with_value("payment_method", RenderValue::text("card"))?
        .with_value("payment_method_label", RenderValue::text("Card"))?
        .with_value("payment_status", RenderValue::text("ready_for_payment"))?
        .with_value("payment_status_label", RenderValue::text("Ready for payment"))?
        .with_value(
            "provider_code",
            RenderValue::text(payment_provider_code(None)),
        )?
        .with_value(
            "provider_label",
            RenderValue::text(payment_provider_label(None).to_string()),
        )?
        .with_value(
            "provider_summary",
            RenderValue::text(payment_provider_summary(None)),
        )?
        .with_value("submit_label", RenderValue::text("Place order"))?
        .with_value("checkout_email", RenderValue::text("member@example.com"))?
        .with_bool("has_checkout_email", true)?
        .with_value("payment_last4", RenderValue::text("4242"))?
        .with_bool("has_payment_reference", true)?
        .with_bool("has_payment_last4", true)?;

    let confirmation = confirmation_model(&current_order)?;

    let customer = RenderModel::new()
        .with_value("display_name", RenderValue::text("Alex Mariner"))?
        .with_value("email", RenderValue::text("member@example.com"))?;

    let recent_orders = vec![
        account_order_from_order(&current_order)?,
        account_order_from_order(&previous_order)?,
    ];
    let membership_summary = membership_summary("Harbor Circle", "Active", "Renews on 18 April")?;

    Ok(StorefrontFixture {
        catalog_sections,
        product_cards: product_cards.clone(),
        product_cards_by_collection,
        product_collection_handles,
        cart_items,
        cart_summary,
        checkout,
        confirmation,
        customer,
        recent_orders,
        membership_summary,
        collections: collections
            .into_iter()
            .zip(
                collections_data
                    .iter()
                    .map(|collection| collection.handle.to_string()),
            )
            .map(|(collection, handle)| (handle, collection))
            .collect(),
        products: product_cards
            .into_iter()
            .zip(
                products_data
                    .iter()
                    .map(|product| product.handle.to_string()),
            )
            .map(|(product, handle)| (handle, product))
            .collect(),
    })
}

fn catalog_admin_form_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = !errors.is_empty();
    RenderModel::new()
        .with_bool("has_errors", has_errors)?
        .with_value(
            "error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| {
                        "Fix the highlighted catalog fields and save again.".to_string()
                    }),
            ),
        )?
        .with_list("errors", errors)
}

fn catalog_admin_products_model(
    locale: &str,
    catalog: &StorefrontCatalog,
    plan: Option<&RuntimePlan>,
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    catalog
        .products
        .iter()
        .map(|product| {
            let collection_name = catalog
                .collection(product.collection_handle.as_str())
                .map(|collection| collection.title.clone())
                .unwrap_or_else(|| "Collection".to_string());
            let fixture = ProductFixture {
                handle: product.handle.clone(),
                title: product.title.clone(),
                summary: product.summary.clone(),
                price: money_display_minor(product.price_minor, &product.currency),
                collection_handle: product.collection_handle.clone(),
                collection_name,
            };
            let is_active_form = catalog_admin_form_targets_product(form_state, &product.handle);
            let title_error = catalog_admin_form_error(form_state, is_active_form, "product_title");
            let summary_error =
                catalog_admin_form_error(form_state, is_active_form, "product_summary");
            let price_error = catalog_admin_form_error(form_state, is_active_form, "product_price");
            let collection_error =
                catalog_admin_form_error(form_state, is_active_form, "product_collection_handle");
            let visibility_input = catalog_admin_checkbox_value(
                form_state,
                is_active_form,
                "product_visible",
                product.is_visible,
            );
            product_model_for_locale(locale, &fixture, plan)?
                .with_bool("is_visible", product.is_visible)?
                .with_bool("visibility_input", visibility_input)?
                .with_value(
                    "visibility_label",
                    RenderValue::text(if product.is_visible {
                        "Visible in storefront"
                    } else {
                        "Hidden from storefront"
                    }),
                )?
                .with_value(
                    "title_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_title",
                        &product.title,
                    )),
                )?
                .with_value(
                    "summary_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_summary",
                        &product.summary,
                    )),
                )?
                .with_value(
                    "price_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_price",
                        &decimal_money_input_minor(product.price_minor),
                    )),
                )?
                .with_value(
                    "collection_handle_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "product_collection_handle",
                        &product.collection_handle,
                    )),
                )?
                .with_bool("has_title_error", title_error.is_some())?
                .with_value(
                    "title_error",
                    RenderValue::text(title_error.unwrap_or_default()),
                )?
                .with_bool("has_summary_error", summary_error.is_some())?
                .with_value(
                    "summary_error",
                    RenderValue::text(summary_error.unwrap_or_default()),
                )?
                .with_bool("has_price_error", price_error.is_some())?
                .with_value(
                    "price_error",
                    RenderValue::text(price_error.unwrap_or_default()),
                )?
                .with_bool("has_collection_error", collection_error.is_some())?
                .with_value(
                    "collection_error",
                    RenderValue::text(collection_error.unwrap_or_default()),
                )?
                .with_list(
                    "collection_options",
                    catalog_admin_collection_options(
                        catalog,
                        catalog_admin_form_value(
                            form_state,
                            is_active_form,
                            "product_collection_handle",
                            &product.collection_handle,
                        )
                        .as_str(),
                    )?,
                )
        })
        .collect()
}

fn catalog_admin_collections_model(
    locale: &str,
    catalog: &StorefrontCatalog,
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    catalog
        .collections
        .iter()
        .map(|collection| {
            let fixture = CollectionFixture {
                handle: collection.handle.clone(),
                title: collection.title.clone(),
                href: localized_collection_path(locale, &collection.handle),
                summary: collection.summary.clone(),
                label: collection.label.clone(),
            };
            let is_active_form =
                catalog_admin_form_targets_collection(form_state, &collection.handle);
            let title_error =
                catalog_admin_form_error(form_state, is_active_form, "collection_title");
            let label_error =
                catalog_admin_form_error(form_state, is_active_form, "collection_label");
            let summary_error =
                catalog_admin_form_error(form_state, is_active_form, "collection_summary");
            let visibility_input = catalog_admin_checkbox_value(
                form_state,
                is_active_form,
                "collection_visible",
                collection.is_visible,
            );
            collection_section_model(locale, &fixture)?
                .with_bool("is_visible", collection.is_visible)?
                .with_bool("visibility_input", visibility_input)?
                .with_value(
                    "visibility_label",
                    RenderValue::text(if collection.is_visible {
                        "Visible in storefront"
                    } else {
                        "Hidden from storefront"
                    }),
                )?
                .with_value(
                    "title_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "collection_title",
                        &collection.title,
                    )),
                )?
                .with_value(
                    "label_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "collection_label",
                        &collection.label,
                    )),
                )?
                .with_value(
                    "summary_input",
                    RenderValue::text(catalog_admin_form_value(
                        form_state,
                        is_active_form,
                        "collection_summary",
                        &collection.summary,
                    )),
                )?
                .with_bool("has_title_error", title_error.is_some())?
                .with_value(
                    "title_error",
                    RenderValue::text(title_error.unwrap_or_default()),
                )?
                .with_bool("has_label_error", label_error.is_some())?
                .with_value(
                    "label_error",
                    RenderValue::text(label_error.unwrap_or_default()),
                )?
                .with_bool("has_summary_error", summary_error.is_some())?
                .with_value(
                    "summary_error",
                    RenderValue::text(summary_error.unwrap_or_default()),
                )
        })
        .collect()
}

fn catalog_admin_collection_options(
    catalog: &StorefrontCatalog,
    selected_handle: &str,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    catalog
        .collections
        .iter()
        .map(|collection| {
            RenderModel::new()
                .with_value("handle", RenderValue::text(collection.handle.clone()))?
                .with_value("title", RenderValue::text(collection.title.clone()))?
                .with_bool("selected", collection.handle == selected_handle)
        })
        .collect()
}

fn catalog_admin_form_targets_product(
    form_state: Option<&StorefrontFormState>,
    handle: &str,
) -> bool {
    catalog_admin_form_matches(form_state, "product", "product_handle", handle)
}

fn catalog_admin_form_targets_collection(
    form_state: Option<&StorefrontFormState>,
    handle: &str,
) -> bool {
    catalog_admin_form_matches(form_state, "collection", "collection_handle", handle)
}

fn catalog_admin_form_matches(
    form_state: Option<&StorefrontFormState>,
    entity: &str,
    handle_field: &str,
    handle: &str,
) -> bool {
    form_state.is_some_and(|state| {
        state.fields.get("catalog_entity").map(String::as_str) == Some(entity)
            && state.fields.get(handle_field).map(String::as_str) == Some(handle)
    })
}

fn catalog_admin_form_value(
    form_state: Option<&StorefrontFormState>,
    is_active_form: bool,
    field: &str,
    default: &str,
) -> String {
    if !is_active_form {
        return default.to_string();
    }
    form_state
        .and_then(|state| state.fields.get(field))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn catalog_admin_checkbox_value(
    form_state: Option<&StorefrontFormState>,
    is_active_form: bool,
    field: &str,
    default: bool,
) -> bool {
    if !is_active_form {
        return default;
    }
    form_state
        .and_then(|state| state.fields.get(field))
        .is_some_and(|value| value == "yes")
}

fn catalog_admin_form_error(
    form_state: Option<&StorefrontFormState>,
    is_active_form: bool,
    field: &str,
) -> Option<String> {
    if !is_active_form {
        return None;
    }
    form_state.and_then(|state| state.field_errors.get(field).cloned())
}

struct CollectionFixture {
    handle: String,
    title: String,
    href: String,
    summary: String,
    label: String,
}

struct ProductFixture {
    handle: String,
    title: String,
    summary: String,
    price: String,
    collection_handle: String,
    collection_name: String,
}

fn collection_section_model(
    locale: &str,
    collection: &CollectionFixture,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("handle", RenderValue::text(collection.handle.as_str()))?
        .with_value("label", RenderValue::text(collection.label.as_str()))?
        .with_value("title", RenderValue::text(collection.title.as_str()))?
        .with_value("summary", RenderValue::text(collection.summary.as_str()))?
        .with_value(
            "url",
            RenderValue::text(localized_collection_path(
                locale,
                collection.handle.as_str(),
            )),
        )
}

fn collection_detail_model(
    locale: &str,
    collection: &CollectionFixture,
    products: &[ProductFixture],
) -> Result<RenderModel, TemplateModelError> {
    let filtered_products = products
        .iter()
        .filter(|product| {
            product.collection_handle == collection.handle || collection.handle == "featured"
        })
        .map(|product| product_model_for_locale(locale, product, None))
        .collect::<Vec<_>>();
    let filtered_products = filtered_products
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    RenderModel::new()
        .with_value("title", RenderValue::text(collection.title.as_str()))?
        .with_value("summary", RenderValue::text(collection.summary.as_str()))?
        .with_value(
            "url",
            RenderValue::text(localized_collection_path(
                locale,
                collection.handle.as_str(),
            )),
        )?
        .with_list("products", filtered_products)
}

fn product_model(product: &ProductFixture) -> Result<RenderModel, TemplateModelError> {
    product_model_for_locale("en-GB", product, None)
}

fn product_model_for_locale(
    locale: &str,
    product: &ProductFixture,
    plan: Option<&RuntimePlan>,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("handle", RenderValue::text(product.handle.as_str()))?
        .with_value("slug", RenderValue::text(product.handle.as_str()))?
        .with_value("sku", RenderValue::text(product.handle.as_str()))?
        .with_value("name", RenderValue::text(product.title.as_str()))?
        .with_value("summary", RenderValue::text(product.summary.as_str()))?
        .with_value("price", RenderValue::text(product.price.as_str()))?
        .with_value(
            "url",
            RenderValue::text(localized_product_path(locale, product.handle.as_str())),
        )?
        .with_value("add_to_cart_url", RenderValue::text("/cart/items"))?
        .with_value(
            "image_url",
            RenderValue::text(storefront_product_image_url(product.handle.as_str(), plan)),
        )?
        .with_value("image_alt", RenderValue::text(product.title.as_str()))?
        .with_value(
            "collection_handle",
            RenderValue::text(product.collection_handle.as_str()),
        )?
        .with_value(
            "collection_url",
            RenderValue::text(localized_collection_path(
                locale,
                product.collection_handle.as_str(),
            )),
        )?
        .with_value(
            "collection_name",
            RenderValue::text(product.collection_name.as_str()),
        )
}

fn storefront_product_image_url(handle: &str, plan: Option<&RuntimePlan>) -> String {
    let remote = match handle {
        "harbor-cap" => Some(
            "https://unsplash.com/photos/a-rack-of-shirts-and-pants-hanging-on-a-clothes-rack-1pT3rOWL_hI/download?force=true&w=1200&q=80",
        ),
        "gold-membership" => Some(
            "https://unsplash.com/photos/woman-in-colorful-outfit-and-fur-coat-DxSHu4GI0Ao/download?force=true&w=1200&q=80",
        ),
        "tasting-pass" => Some(
            "https://unsplash.com/photos/people-browsing-clothing-racks-in-a-well-lit-store-oOAYziRlpMw/download?force=true&w=1200&q=80",
        ),
        "harbor-scarf" => Some(
            "https://unsplash.com/photos/woman-wearing-gray-coat-CKxpOhAoSRg/download?force=true&w=1200&q=80",
        ),
        "brooklyn-night-pass" => Some(
            "https://unsplash.com/photos/modern-luxury-store-interior-with-display-shelves-and-seating-8YDqTT5jNXI/download?force=true&w=1200&q=80",
        ),
        _ => None,
    };

    remote
        .map(str::to_string)
        .unwrap_or_else(|| theme_asset_url(plan, "theme/assets/logo.svg"))
}

fn theme_asset_url(plan: Option<&RuntimePlan>, logical_path: &str) -> String {
    plan.and_then(|runtime| runtime.theme_asset_manifest.as_ref())
        .and_then(|manifest| manifest.resolve(logical_path))
        .and_then(|published| match published.delivery().target() {
            AssetDeliveryTarget::Cdn { public_url, .. } => Some(public_url.clone()),
            _ => None,
        })
        .unwrap_or_else(|| format!("/{logical_path}"))
}

fn cart_item(
    title: &str,
    variant: &str,
    quantity: &str,
    total: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("variant", RenderValue::text(variant))?
        .with_value("quantity", RenderValue::text(quantity))?
        .with_value(
            "quantity_field",
            RenderValue::text(format!(
                "quantity_{}",
                title.to_lowercase().replace(' ', "-")
            )),
        )?
        .with_value("total", RenderValue::text(total))
}

fn account_order(
    reference: &str,
    total: &str,
    status: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("reference", RenderValue::text(reference))?
        .with_value("total", RenderValue::text(total))?
        .with_value("status", RenderValue::text(status))
}

fn cart_item_from_line(
    catalog: &StorefrontCatalog,
    locale: &str,
    line: &CheckoutLine,
) -> Result<RenderModel, TemplateModelError> {
    let model = cart_item(
        &line.product_title,
        &line.variant_title,
        &line.quantity.to_string(),
        &money_display(&line.subtotal().expect("sample checkout line is valid")),
    )?;
    decorate_cart_item_with_catalog_context(
        model,
        catalog,
        locale,
        line.sku.as_str(),
        &line.product_title,
    )
}

fn confirmation_model(order: &Order) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("order_number", RenderValue::text(order.id.to_string()))?
        .with_value("email", RenderValue::text("member@example.com"))?
        .with_bool("has_email", true)?
        .with_value("next_step", RenderValue::text(order.confirmation_message()))
        .and_then(|model| {
            model.with_value("status", RenderValue::text(order.history_status_label()))
        })
        .and_then(|model| {
            model.with_value(
                "subtotal",
                RenderValue::text(money_display(&order.totals.subtotal)),
            )
        })
        .and_then(|model| model.with_value("total", RenderValue::text(order.display_total())))
        .and_then(|model| model.with_value("payment_status", RenderValue::text("Captured")))
        .and_then(|model| model.with_value("payment_method", RenderValue::text("Card")))
        .and_then(|model| model.with_value("payment_reference", RenderValue::text("PAY-50001")))
        .and_then(|model| model.with_value("payment_last4", RenderValue::text("4242")))
        .and_then(|model| {
            model.with_value(
                "payment_summary",
                RenderValue::text("Card ending 4242, reference PAY-50001"),
            )
        })
        .and_then(|model| {
            model.with_value(
                "provider_label",
                RenderValue::text(payment_provider_label(None).to_string()),
            )
        })
        .and_then(|model| model.with_bool("has_payment_last4", true))
        .and_then(|model| model.with_bool("has_payment_reference", true))
        .and_then(|model| {
            model.with_bool(
                "has_membership_items",
                order.outcomes().iter().any(|outcome| {
                    matches!(
                        outcome,
                        coil_commerce::OrderOutcome::GrantMembership {
                            entitlement_key: _,
                            quantity: _
                        }
                    )
                }),
            )
        })
        .and_then(|model| model.with_bool("has_line_items", !order.lines.is_empty()))
        .and_then(|model| {
            model.with_list(
                "line_items",
                order
                    .lines
                    .iter()
                    .map(|line| {
                        RenderModel::new()
                            .with_value("title", RenderValue::text(line.product_title.clone()))?
                            .with_value("quantity", RenderValue::text(line.quantity.to_string()))?
                            .with_value(
                                "total",
                                RenderValue::text(money_display(
                                    &line.subtotal().expect("fixture confirmation subtotal"),
                                )),
                            )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        })
}

fn account_order_from_order(order: &Order) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("reference", RenderValue::text(order.id.to_string()))?
        .with_value("total", RenderValue::text(order.display_total()))?
        .with_value("status", RenderValue::text(order.history_status_label()))
}

fn membership_summary(
    tier_name: &str,
    status: &str,
    renewal_text: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("tier_name", RenderValue::text(tier_name))?
        .with_value("status", RenderValue::text(status))?
        .with_value("renewal_text", RenderValue::text(renewal_text))
}

fn sample_completed_order() -> Order {
    let currency = CurrencyCode::new("GBP").unwrap();
    let pricing = PricingPolicy::new(currency.clone());
    let mut checkout =
        CheckoutSession::new(CheckoutId::new("chk-10042").unwrap(), currency.clone());
    checkout
        .add_line(
            CheckoutLine::new(
                ProductId::new("product-harbor-cap").unwrap(),
                ProductKind::Physical,
                "Harbor Cap",
                Sku::new("sku-harbor-cap").unwrap(),
                "Canvas cap",
                1,
                Money::new(currency.clone(), 2_900).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    checkout
        .add_line(
            CheckoutLine::new(
                ProductId::new("product-gold-membership").unwrap(),
                ProductKind::Membership {
                    entitlement_key: EntitlementKey::new("membership.gold").unwrap(),
                },
                "Gold Membership",
                Sku::new("sku-gold-membership").unwrap(),
                "Annual plan",
                1,
                Money::new(currency.clone(), 8_900).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();
    checkout
        .finalize(OrderId::new("ORD-10042").unwrap(), &pricing)
        .unwrap()
}

fn sample_previous_order() -> Order {
    let currency = CurrencyCode::new("GBP").unwrap();
    let pricing = PricingPolicy::new(currency.clone());
    let mut checkout = CheckoutSession::new(CheckoutId::new("chk-0998").unwrap(), currency.clone());
    checkout
        .add_line(
            CheckoutLine::new(
                ProductId::new("product-spring-tasting-pass").unwrap(),
                ProductKind::Service,
                "Spring Tasting Pass",
                Sku::new("sku-tasting-pass").unwrap(),
                "Single event pass",
                1,
                Money::new(currency.clone(), 4_500).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    checkout.ready_for_payment().unwrap();
    checkout.awaiting_payment().unwrap();
    checkout.mark_paid().unwrap();
    let mut order = checkout
        .finalize(OrderId::new("ORD-0998").unwrap(), &pricing)
        .unwrap();
    order.fulfill().unwrap();
    order
}

fn money_display(money: &Money) -> String {
    money_display_minor(money.amount_minor(), money.currency().as_str())
}

fn decimal_money_input_minor(amount_minor: i64) -> String {
    let major = amount_minor / 100;
    let remainder = amount_minor.abs() % 100;
    format!("{major}.{remainder:02}")
}

fn money_display_minor(amount_minor: i64, currency: &str) -> String {
    let major = amount_minor / 100;
    let remainder = amount_minor % 100;
    match currency {
        "GBP" => format!("£{major}.{remainder:02}"),
        code => format!("{code} {major}.{remainder:02}"),
    }
}

fn empty_membership_summary() -> Result<RenderModel, TemplateModelError> {
    membership_summary(
        "Membership unavailable",
        "Not active",
        "Join from the storefront to manage renewals and entitlements here.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::RuntimeBuilder;
    use coil_auth::DefaultAuthModelPackage;
    use coil_commerce::CommerceModule;
    use coil_config::PlatformConfig;
    use coil_customer_sdk::{
        AuditFacade, AuthFacade, BackendError, BackendErrorKind, CheckoutHooks, CommerceFacade,
        CustomerBackendPlugin, CustomerHookRegistry, CustomerPluginDescriptor, MergePolicy,
        OrderDraft, OrderReviewDecision, RenderModelContribution, RenderModelHooks, RenderTarget,
        RepositoryFacade, RequestContext,
    };
    use coil_template::{
        DocumentRenderRequest, TemplateName, TemplateNamespace, TemplateRegistry, TemplateRuntime,
        TemplateSelector, TemplateSourceParser,
    };
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    const RENDER_TEST_CONFIG: &str = r#"
[app]
name = "customer-render-tests"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "coil_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "coil_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/tmp/coil-runtime-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "coil-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms-pages", "admin-shell"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#;

    #[derive(Debug)]
    struct NoteRecordingCheckoutPlugin;

    #[derive(Debug)]
    struct NoteRecordingCheckoutHooks;

    impl CheckoutHooks for NoteRecordingCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            order: &OrderDraft,
            commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            commerce.add_order_note(&order.order_id, "Flag for finance follow-up")?;
            commerce.add_order_note(&order.order_id, "Flag for finance follow-up")?;
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CustomerBackendPlugin for NoteRecordingCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-note-recorder",
                "Render Order Note Recorder",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(NoteRecordingCheckoutHooks))
        }
    }

    #[derive(Debug)]
    struct WrongOrderNoteCheckoutPlugin;

    #[derive(Debug)]
    struct WrongOrderNoteCheckoutHooks;

    #[derive(Debug)]
    struct MetadataReplayCheckoutPlugin;

    #[derive(Debug)]
    struct MetadataReplayCheckoutHooks;

    #[derive(Debug)]
    struct StoredPrincipalReplayCheckoutPlugin;

    #[derive(Debug)]
    struct StoredPrincipalReplayCheckoutHooks;

    #[derive(Debug)]
    struct ViewerFallbackPrincipalReplayCheckoutPlugin;

    #[derive(Debug)]
    struct ViewerFallbackPrincipalReplayCheckoutHooks;

    #[derive(Debug)]
    struct AdjustedMetadataCheckoutPlugin;

    #[derive(Debug)]
    struct AdjustedMetadataCheckoutHooks;

    #[derive(Debug)]
    struct ReplayPrincipalCheckoutPlugin;

    #[derive(Debug)]
    struct ReplayPrincipalCheckoutHooks;

    #[derive(Debug)]
    struct TargetAwareRenderModelPlugin;

    #[derive(Debug)]
    struct TargetAwareRenderModelHooks;

    impl CheckoutHooks for WrongOrderNoteCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            _order: &OrderDraft,
            commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            commerce.add_order_note("ORD-OTHER", "This should fail closed")?;
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CustomerBackendPlugin for WrongOrderNoteCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-note-mismatch",
                "Render Order Note Mismatch",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(WrongOrderNoteCheckoutHooks))
        }
    }

    impl CheckoutHooks for MetadataReplayCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            let membership_tier = order.metadata.get("membership_tier").map(String::as_str);
            let shipping_country = order.metadata.get("shipping_country").map(String::as_str);
            let order_principal_id = order.metadata.get("order_principal_id").map(String::as_str);
            if membership_tier != Some("gold")
                || shipping_country != Some("GB")
                || order_principal_id != Some("member@example.com")
            {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "checkout.metadata.missing",
                    "Render replay lost linked checkout metadata.",
                ));
            }
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CheckoutHooks for StoredPrincipalReplayCheckoutHooks {
        fn review_order(
            &self,
            ctx: &RequestContext,
            _order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            if ctx.principal.id.as_deref() != Some("member@example.com") {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "checkout.principal.mismatch",
                    "Render replay did not use the stored order principal.",
                ));
            }
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CheckoutHooks for ViewerFallbackPrincipalReplayCheckoutHooks {
        fn review_order(
            &self,
            ctx: &RequestContext,
            _order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            if ctx.principal.id.as_deref() != Some("operator@example.com") {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "checkout.principal.fallback_mismatch",
                    "Render replay did not fall back to the current viewer principal.",
                ));
            }
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CheckoutHooks for AdjustedMetadataCheckoutHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            _order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            Ok(OrderReviewDecision::Adjusted(
                coil_customer_sdk::OrderAdjustment::new(
                    "Gold high-value order: route to concierge packing and same-day follow-up.",
                )
                .with_metadata_entries([
                    ("assigned_queue", "vip-fulfilment"),
                    ("service_level", "priority"),
                    (
                        "tags",
                        "customer-app:shoppr,queue:vip-fulfilment,service-level:priority",
                    ),
                ]),
            ))
        }
    }

    impl CustomerBackendPlugin for MetadataReplayCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-metadata-replay",
                "Render Order Metadata Replay",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(MetadataReplayCheckoutHooks))
        }
    }

    impl CustomerBackendPlugin for AdjustedMetadataCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-adjusted-metadata",
                "Render Order Adjusted Metadata",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(AdjustedMetadataCheckoutHooks))
        }
    }

    impl CustomerBackendPlugin for StoredPrincipalReplayCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-stored-principal-replay",
                "Render Order Stored Principal Replay",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(StoredPrincipalReplayCheckoutHooks))
        }
    }

    impl CustomerBackendPlugin for ViewerFallbackPrincipalReplayCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-viewer-principal-fallback",
                "Render Order Viewer Principal Fallback",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(ViewerFallbackPrincipalReplayCheckoutHooks))
        }
    }

    impl CheckoutHooks for ReplayPrincipalCheckoutHooks {
        fn review_order(
            &self,
            ctx: &RequestContext,
            order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            let replay_principal_id = ctx.principal.id.as_deref();
            let stored_principal_id = order.metadata.get("order_principal_id").map(String::as_str);
            if replay_principal_id != stored_principal_id {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "checkout.replay.principal_mismatch",
                    "Render replay should use the stored order principal.",
                ));
            }
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CustomerBackendPlugin for ReplayPrincipalCheckoutPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "render-order-replay-principal",
                "Render Order Replay Principal",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_checkout_hooks(Arc::new(ReplayPrincipalCheckoutHooks))
        }
    }

    impl RenderModelHooks for TargetAwareRenderModelHooks {
        fn contribute_render_model(
            &self,
            _ctx: &RequestContext,
            target: &RenderTarget,
            _repositories: &dyn RepositoryFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<Vec<RenderModelContribution>, BackendError> {
            let mounted = RenderModel::new()
                .with_value("route_name", RenderValue::text(target.route_name.clone()))
                .and_then(|model| {
                    model.with_value("template_name", RenderValue::text(target.template_name.clone()))
                })
                .and_then(|model| {
                    model.with_value(
                        "product_slug",
                        RenderValue::text(
                            target
                                .route_params
                                .get("product_slug")
                                .cloned()
                                .unwrap_or_default(),
                        ),
                    )
                })
                .and_then(|model| {
                    model.with_value(
                        "view",
                        RenderValue::text(
                            target.query_params.get("view").cloned().unwrap_or_default(),
                        ),
                    )
                })
                .map_err(|error| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "render_model.target.invalid",
                        error.to_string(),
                    )
                })?;
            let page_overlay = RenderModel::new()
                .with_value("render_source", RenderValue::text("linked-rust"))
                .map_err(|error| {
                    BackendError::new(
                        BackendErrorKind::Internal,
                        "render_model.target.invalid",
                        error.to_string(),
                    )
                })?;
            Ok(vec![
                RenderModelContribution::mount("customer_extension", mounted)?,
                RenderModelContribution::merge("page", page_overlay, MergePolicy::FailOnConflict)?,
            ])
        }
    }

    impl CustomerBackendPlugin for TargetAwareRenderModelPlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new(
                "customer-render-model-target-aware",
                "Customer Render Model Target Aware",
                "0.1.0",
            )
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            registry.register_render_model_hooks(Arc::new(TargetAwareRenderModelHooks))
        }
    }

    fn render_test_plan_with_customer_plugin<C>(plugin: C) -> RuntimePlan
    where
        C: CustomerBackendPlugin,
    {
        let app_name = format!(
            "render-customer-hooks-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let storage_root = std::env::temp_dir().join(&app_name).display().to_string();
        let config = PlatformConfig::from_toml_str(
            &RENDER_TEST_CONFIG
                .replace(
                    "name = \"customer-render-tests\"",
                    &format!("name = \"{app_name}\""),
                )
                .replace(
                    "local_root = \"var/storage\"",
                    &format!("local_root = \"{storage_root}\""),
                ),
        )
        .unwrap();
        RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_customer_plugin(plugin)
            .build()
            .unwrap()
    }

    fn sample_storefront_order_snapshot() -> StorefrontOrderSnapshot {
        StorefrontOrderSnapshot {
            order_id: "ORD-10042".to_string(),
            session_id: "session-order-detail".to_string(),
            principal_id: Some("member@example.com".to_string()),
            metadata: BTreeMap::from([
                ("membership_tier".to_string(), "gold".to_string()),
                ("shipping_country".to_string(), "GB".to_string()),
                ("expedited_requested".to_string(), "false".to_string()),
            ]),
            status: "paid".to_string(),
            payment: StorefrontPaymentSnapshot {
                status: "captured".to_string(),
                method: Some("card".to_string()),
                reference: Some("PAY-50001".to_string()),
                last4: Some("4242".to_string()),
                checkout_email: Some("member@example.com".to_string()),
            },
            currency: "GBP".to_string(),
            line_count: 1,
            subtotal_minor: 8_900,
            total_minor: 8_900,
            refunded_total_minor: 0,
            refundable_total_minor: 8_900,
            subtotal: "£89.00".to_string(),
            total: "£89.00".to_string(),
            refunded_total: "£0.00".to_string(),
            refundable_total: "£89.00".to_string(),
            created_at_unix_seconds: 0,
            lines: vec![StorefrontOrderLine {
                sku: "gold-membership".to_string(),
                title: "Gold Membership".to_string(),
                variant_title: "Annual".to_string(),
                product_kind: "membership".to_string(),
                entitlement_key: Some("membership.gold".to_string()),
                metadata: BTreeMap::from([("variant_title".to_string(), "Annual".to_string())]),
                quantity: 1,
                unit_price_minor: 8_900,
                total_minor: 8_900,
                currency: "GBP".to_string(),
                total: "£89.00".to_string(),
            }],
            refunds: Vec::new(),
        }
    }

    fn fixture_model(route_name: &str) -> RenderModel {
        apply_route_specific_bindings(
            None,
            RenderModel::new(),
            route_name,
            None,
            "en-GB",
            &BTreeMap::new(),
            &RequestFieldMap::new(),
            None,
            None,
            None,
        )
        .unwrap()
    }

    fn live_account_model(principal_id: &str) -> RenderModel {
        let session = SessionContext {
            session_id: Some("session-live-123".to_string()),
            resolved_from_cookie: true,
        };
        let principal = PrincipalContext {
            principal_id: Some(principal_id.to_string()),
            principal_kind: RequestPrincipalKind::User,
            granted_capabilities: HashSet::new(),
        };
        apply_route_specific_bindings(
            None,
            RenderModel::new(),
            "memberships.account",
            None,
            "en-GB",
            &BTreeMap::new(),
            &RequestFieldMap::new(),
            None,
            Some(&session),
            Some(&principal),
        )
        .unwrap()
    }

    fn session_scoped_account_model() -> RenderModel {
        let session = SessionContext {
            session_id: Some("session-live-guest".to_string()),
            resolved_from_cookie: true,
        };
        apply_route_specific_bindings(
            None,
            RenderModel::new(),
            "memberships.account.dashboard",
            None,
            "en-GB",
            &BTreeMap::new(),
            &RequestFieldMap::new(),
            None,
            Some(&session),
            None,
        )
        .unwrap()
    }

    fn render_fixture(route_name: &str, template_body: &str) -> String {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                template_body,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    fixture_model(route_name),
                ),
            )
            .unwrap()
            .html
    }

    #[test]
    fn route_specific_model_populates_catalog_listing() {
        let html = render_fixture(
            "commerce.catalog",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <ul>
      <li coil:each="section : catalog_sections" coil:text="${section.title}">Fallback</li>
    </ul>
  </body>
</html>"#,
        );

        assert!(html.contains("Featured"));
        assert!(html.contains("Memberships"));
    }

    #[test]
    fn route_specific_model_populates_checkout_intent_fields() {
        let html = render_fixture(
            "commerce.checkout",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="provider" coil:text="${checkout.provider_label}">Provider</p>
    <p class="status" coil:text="${checkout.payment_status_label}">Ready</p>
    <p class="reference" coil:text="${checkout.payment_reference}">PAYMENT-PENDING</p>
    <p class="summary" coil:text="${checkout.provider_summary}">Summary</p>
    <input type="hidden" name="payment_method" coil:attr="value=${checkout.payment_method}" />
  </body>
</html>"#,
        );

        assert!(html.contains("Platform fallback payment path"));
        assert!(html.contains("Ready for payment"));
        assert!(html.contains("card-on-file"));
        assert!(html.contains("provider-backed handoff"));
        assert!(html.contains("value=\"card\""));
    }

    #[test]
    fn route_specific_model_populates_checkout_confirmation_payment_fields() {
        let html = render_fixture(
            "commerce.checkout-confirmation",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="status" coil:text="${confirmation.status}">Paid</p>
    <p class="total" coil:text="${confirmation.total}">£0.00</p>
    <p class="payment-summary" coil:text="${confirmation.payment_summary}">Summary</p>
    <p class="provider" coil:text="${confirmation.provider_label}">Provider</p>
  </body>
</html>"#,
        );

        assert!(html.contains("Paid"));
        assert!(html.contains("£118.00"));
        assert!(html.contains("Card ending 4242, reference PAY-50001"));
        assert!(html.contains("Platform fallback payment path"));
    }

    #[test]
    fn route_specific_model_populates_account_surface() {
        let html = render_fixture(
            "memberships.account",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <h1 coil:text="${customer.display_name}">Fallback</h1>
    <p coil:text="${membership_summary.tier_name}">Tier</p>
  </body>
</html>"#,
        );

        assert!(html.contains("Alex Mariner"));
        assert!(html.contains("Harbor Circle"));
    }

    #[test]
    fn route_specific_model_populates_checkout_surface() {
        let html = render_fixture(
            "commerce.checkout",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <input type="text" coil:attr="value=${checkout.payment_reference}" value="fallback" />
    <strong coil:text="${customer.email}">Fallback</strong>
  </body>
</html>"#,
        );

        assert!(html.contains("card-on-file"));
        assert!(html.contains("member@example.com"));
    }

    #[test]
    fn route_specific_model_populates_product_slug_and_related_cards() {
        let html = render_fixture(
            "commerce.product-detail",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <input type="hidden" coil:attr="value=${product.slug}" value="fallback" />
    <ul>
      <li coil:each="product : ${product_cards}" coil:text="${product.slug}">fallback</li>
    </ul>
  </body>
</html>"#,
        );

        assert!(html.contains("value=\"harbor-cap\""), "{html}");
        assert!(html.contains("gold-membership"), "{html}");
    }

    #[test]
    fn route_specific_model_populates_cart_links_from_fixture_catalog() {
        let html = render_fixture(
            "commerce.cart",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <ul>
      <li coil:each="item : ${cart_items}">
        <a class="collection" coil:if="${item.has_product_link}" coil:attr="href=${item.collection_url}" coil:text="${item.collection_name}">Collection</a>
        <a class="product" coil:if="${item.has_product_link}" coil:attr="href=${item.product_url}" coil:text="${item.title}">Product</a>
      </li>
    </ul>
  </body>
</html>"#,
        );

        assert!(html.contains("/en-GB/shop/products/harbor-cap"), "{html}");
        assert!(html.contains("/en-GB/shop/collections/featured"), "{html}");
        assert!(
            html.contains("/en-GB/shop/collections/memberships"),
            "{html}"
        );
        assert!(html.contains("Gold Membership"), "{html}");
    }

    #[test]
    fn live_account_surface_prefers_session_backed_customer_state() {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <h1 coil:text="${customer.display_name}">Fallback</h1>
    <p class="summary" coil:text="${account.state_summary}">State</p>
    <p class="email" coil:if="${account.has_customer_email}" coil:text="${customer.email}">Email</p>
    <p class="latest-order" coil:if="${account.has_latest_order}">
      <strong coil:text="${account.latest_order_reference}">Order</strong>
      <span coil:text="${account.latest_order_status}">Status</span>
    </p>
    <ul class="orders">
      <li coil:each="order : ${recent_orders}">
        <strong coil:text="${order.reference}">Order</strong>
        <span coil:text="${order.status}">Status</span>
        <span coil:text="${order.total}">Total</span>
      </li>
    </ul>
    <p class="membership" coil:text="${membership_summary.tier_name}">Membership</p>
    <p class="membership-status" coil:text="${membership_summary.status}">Active</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    live_account_model("sea.member@example.com"),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Sea Member"));
        assert!(html.contains("sea.member@example.com"));
        assert!(html.contains("live storefront session identity"));
        assert!(!html.contains("ORD-10042"));
        assert!(!html.contains("Paid"));
        assert!(!html.contains("Gold Membership"));
        assert!(html.contains("Membership unavailable"));
    }

    #[test]
    fn session_scoped_account_surface_uses_honest_browser_session_guidance() {
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="summary" coil:text="${account.state_summary}">State</p>
    <p class="orders-empty" coil:text="${account.orders_empty_text}">Orders</p>
    <p class="membership-empty" coil:text="${account.membership_empty_text}">Membership</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    session_scoped_account_model(),
                ),
            )
            .unwrap()
            .html;

        assert!(
            html.contains("follows the current browser session"),
            "{html}"
        );
        assert!(html.contains("no completed orders yet"), "{html}");
        assert!(html.contains("qualifying membership purchase"), "{html}");
    }

    #[test]
    fn customer_order_review_surfaces_render_recorded_notes() {
        let plan = render_test_plan_with_customer_plugin(NoteRecordingCheckoutPlugin);
        let review = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .unwrap()
            .expect("checkout hook review should exist");

        assert_eq!(review.notes, vec!["Flag for finance follow-up".to_string()]);

        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="status" coil:text="${review.status}">Approved</p>
    <ul coil:if="${review.has_notes}">
      <li coil:each="note : ${review.notes}" coil:text="${note.text}">note</li>
    </ul>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    RenderModel::new()
                        .with_object("review", customer_order_review_model(review).unwrap())
                        .unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Approved"), "{html}");
        assert!(html.contains("Flag for finance follow-up"), "{html}");
    }

    #[test]
    fn customer_order_review_fails_closed_on_mismatched_render_order_note_target() {
        let plan = render_test_plan_with_customer_plugin(WrongOrderNoteCheckoutPlugin);
        let error = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .expect_err("mismatched note target should fail the render hook");
        let message = error.to_string();

        assert!(message.contains("order_mismatch"), "{message}");
        assert!(message.contains("ORD-10042"), "{message}");
    }

    #[test]
    fn customer_order_review_replays_persisted_order_metadata_into_linked_hooks() {
        let plan = render_test_plan_with_customer_plugin(MetadataReplayCheckoutPlugin);
        let review = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .unwrap()
            .expect("metadata-aware review should exist");

        assert!(matches!(review.decision, OrderReviewDecision::Approved));
    }

    #[test]
    fn customer_order_review_prefers_stored_order_principal_over_current_viewer() {
        let plan = render_test_plan_with_customer_plugin(StoredPrincipalReplayCheckoutPlugin);
        let operator = PrincipalContext {
            principal_id: Some("operator@example.com".to_string()),
            principal_kind: RequestPrincipalKind::User,
            granted_capabilities: HashSet::new(),
        };
        let review = customer_order_review(
            &plan,
            &sample_storefront_order_snapshot(),
            None,
            Some(&operator),
        )
        .unwrap()
        .expect("stored principal review should exist");

        assert!(matches!(review.decision, OrderReviewDecision::Approved));
    }

    #[test]
    fn customer_order_review_falls_back_to_viewer_principal_when_order_identity_is_absent() {
        let plan =
            render_test_plan_with_customer_plugin(ViewerFallbackPrincipalReplayCheckoutPlugin);
        let operator = PrincipalContext {
            principal_id: Some("operator@example.com".to_string()),
            principal_kind: RequestPrincipalKind::User,
            granted_capabilities: HashSet::new(),
        };
        let mut order = sample_storefront_order_snapshot();
        order.principal_id = None;
        let review = customer_order_review(&plan, &order, None, Some(&operator))
            .unwrap()
            .expect("viewer fallback review should exist");

        assert!(matches!(review.decision, OrderReviewDecision::Approved));
    }

    #[test]
    fn customer_order_review_surfaces_adjustment_metadata() {
        let plan = render_test_plan_with_customer_plugin(AdjustedMetadataCheckoutPlugin);
        let review = customer_order_review(&plan, &sample_storefront_order_snapshot(), None, None)
            .unwrap()
            .expect("adjusted review should exist");
        let review_model = customer_order_review_model(review).unwrap();
        let html = TemplateRuntime::new({
            let namespace = TemplateNamespace::new("customer-app").unwrap();
            let mut registry = TemplateRegistry::new();
            registry
                .register(
                    TemplateSourceParser::new()
                        .parse_layout(
                            namespace.clone(),
                            TemplateName::new("page").unwrap(),
                            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p coil:text="${review.assigned_queue}">vip-fulfilment</p>
    <p coil:text="${review.service_level}">priority</p>
    <ul coil:if="${review.has_metadata}">
      <li coil:each="entry : ${review.metadata_entries}">
        <span coil:text="${entry.key}">assigned_queue</span>
        <span>=</span>
        <span coil:text="${entry.value}">vip-fulfilment</span>
      </li>
    </ul>
  </body>
</html>"#,
                        )
                        .unwrap(),
                )
                .unwrap();
            registry
        })
        .render_document(
            &[TemplateNamespace::new("customer-app").unwrap()],
            DocumentRenderRequest::new(
                TemplateSelector::new(TemplateName::new("page").unwrap()),
                RenderModel::new()
                    .with_object("review", review_model)
                    .unwrap(),
            ),
        )
        .unwrap()
        .html;

        assert!(html.contains("vip-fulfilment"), "{html}");
        assert!(html.contains("priority"), "{html}");
        assert!(html.contains("assigned_queue"), "{html}");
        assert!(html.contains("service_level"), "{html}");
    }

    #[test]
    fn customer_order_review_replays_using_the_stored_order_principal() {
        let plan = render_test_plan_with_customer_plugin(ReplayPrincipalCheckoutPlugin);
        let operator = PrincipalContext {
            principal_id: Some("ops.admin@example.com".to_string()),
            principal_kind: RequestPrincipalKind::User,
            granted_capabilities: HashSet::new(),
        };

        let review = customer_order_review(
            &plan,
            &sample_storefront_order_snapshot(),
            None,
            Some(&operator),
        )
        .unwrap()
        .expect("principal-aware review should exist");

        assert!(matches!(review.decision, OrderReviewDecision::Approved));
    }

    #[test]
    fn render_model_hooks_mount_namespaced_models_and_merge_page_fields() {
        let config = PlatformConfig::from_toml_str(RENDER_TEST_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(CommerceModule::new())
            .with_customer_plugin(TargetAwareRenderModelPlugin)
            .build()
            .unwrap();

        let execution = plan
            .execute_request(
                RequestInput::new(
                    HttpMethod::Get,
                    "www.example.com",
                    "/en-GB/shop/products/harbor-cap",
                )
                .unwrap()
                .with_query_param("view", "summary"),
                b"01234567012345670123456701234567",
                b"76543210765432107654321076543210",
            )
            .unwrap();
        let model = plan
            .render_model_for_execution(&execution, "commerce/product-detail", None)
            .unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="route" coil:text="${customer_extension.route_name}">route</p>
    <p class="template" coil:text="${customer_extension.template_name}">template</p>
    <p class="slug" coil:text="${customer_extension.product_slug}">slug</p>
    <p class="view" coil:text="${customer_extension.view}">view</p>
    <p class="source" coil:text="${page.render_source}">source</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    model,
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("commerce.product-detail"), "{html}");
        assert!(html.contains("commerce/product-detail"), "{html}");
        assert!(html.contains("harbor-cap"), "{html}");
        assert!(html.contains("summary"), "{html}");
        assert!(html.contains("linked-rust"), "{html}");
    }

    #[test]
    fn render_model_hooks_fail_closed_on_merge_conflicts_by_default() {
        let model = RenderModel::new()
            .with_object(
                "page",
                RenderModel::new()
                    .with_value("title", RenderValue::text("Runtime title"))
                    .unwrap(),
            )
            .unwrap();
        let contribution = RenderModelContribution::merge(
            "page",
            RenderModel::new()
                .with_value("title", RenderValue::text("Customer title"))
                .unwrap(),
            MergePolicy::FailOnConflict,
        )
        .unwrap();

        let error = apply_customer_render_model_contribution(model, contribution).unwrap_err();
        let message = error.to_string();

        assert!(message.contains("page.title"), "{message}");
        assert!(message.contains("existing value differs"), "{message}");
    }

    #[test]
    fn render_model_hooks_can_replace_existing_fields() {
        let model = RenderModel::new()
            .with_object(
                "page",
                RenderModel::new()
                    .with_value("title", RenderValue::text("Runtime title"))
                    .unwrap(),
            )
            .unwrap();
        let contribution = RenderModelContribution::merge(
            "page",
            RenderModel::new()
                .with_value("title", RenderValue::text("Customer title"))
                .unwrap(),
            MergePolicy::ReplaceExisting,
        )
        .unwrap();

        let merged = apply_customer_render_model_contribution(model, contribution).unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p coil:text="${page.title}">title</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    merged,
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Customer title"), "{html}");
    }

    #[test]
    fn render_model_hooks_can_append_lists() {
        let model = RenderModel::new()
            .with_object(
                "page",
                RenderModel::new()
                    .with_list(
                        "sections",
                        vec![
                            RenderModel::new()
                                .with_value("label", RenderValue::text("alpha"))
                                .unwrap(),
                        ],
                    )
                    .unwrap(),
            )
            .unwrap();
        let contribution = RenderModelContribution::merge(
            "page",
            RenderModel::new()
                .with_list(
                    "sections",
                    vec![
                        RenderModel::new()
                            .with_value("label", RenderValue::text("beta"))
                            .unwrap(),
                    ],
                )
                .unwrap(),
            MergePolicy::AppendLists,
        )
        .unwrap();

        let merged = apply_customer_render_model_contribution(model, contribution).unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p coil:each="section : ${page.sections}" coil:text="${section.label}">section</p>
  </body>
</html>"#,
            )
            .unwrap();
        let mut registry = TemplateRegistry::new();
        registry.register(template).unwrap();
        let html = TemplateRuntime::new(registry)
            .render_document(
                &[namespace],
                DocumentRenderRequest::new(
                    TemplateSelector::new(TemplateName::new("page").unwrap()),
                    merged,
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("alpha"), "{html}");
        assert!(html.contains("beta"), "{html}");
    }
}
#[derive(Clone)]
struct StorefrontPageFeedback {
    visible_flash_messages: Vec<FlashMessage>,
    form_state: Option<StorefrontFormState>,
}

fn storefront_page_feedback(route_name: &str, messages: &[FlashMessage]) -> StorefrontPageFeedback {
    let mut visible_flash_messages = Vec::new();
    let mut form_state = None;
    for message in messages {
        if let Some(decoded) = StorefrontFormState::decode(&message.text) {
            if decoded.route_name == route_name && form_state.is_none() {
                form_state = Some(decoded);
            }
            continue;
        }
        visible_flash_messages.push(message.clone());
    }
    StorefrontPageFeedback {
        visible_flash_messages,
        form_state,
    }
}

fn cart_form_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = !errors.is_empty();
    RenderModel::new()
        .with_bool("has_errors", has_errors)?
        .with_value(
            "error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| "Fix the highlighted cart lines and try again.".to_string()),
            ),
        )?
        .with_list("errors", errors)
}

fn merge_checkout_form_feedback(
    model: RenderModel,
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let errors = form_errors_model(form_state)?;
    let has_errors = form_state.is_some() || !errors.is_empty();
    let checkout_email_error = storefront_field_error(form_state, "checkout_email");
    let payment_method_error = storefront_field_error(form_state, "payment_method");
    let payment_last4_error = storefront_field_error(form_state, "payment_last4");
    let checkout_intent_error = storefront_field_error(form_state, "checkout_intent");
    let terms_accepted_error = storefront_field_error(form_state, "terms_accepted");
    model
        .with_bool("has_errors", has_errors)?
        .with_value(
            "error_summary",
            RenderValue::text(
                form_state
                    .map(|state| state.summary.clone())
                    .unwrap_or_else(|| {
                        "Review the highlighted checkout fields and try again.".to_string()
                    }),
            ),
        )?
        .with_list("errors", errors)?
        .with_bool("has_checkout_email_error", checkout_email_error.is_some())?
        .with_value(
            "checkout_email_error",
            RenderValue::text(checkout_email_error.unwrap_or_default()),
        )?
        .with_bool("has_payment_method_error", payment_method_error.is_some())?
        .with_value(
            "payment_method_error",
            RenderValue::text(payment_method_error.unwrap_or_default()),
        )?
        .with_bool("has_payment_last4_error", payment_last4_error.is_some())?
        .with_value(
            "payment_last4_error",
            RenderValue::text(payment_last4_error.unwrap_or_default()),
        )?
        .with_bool("has_checkout_intent_error", checkout_intent_error.is_some())?
        .with_value(
            "checkout_intent_error",
            RenderValue::text(checkout_intent_error.unwrap_or_default()),
        )?
        .with_bool("has_terms_accepted_error", terms_accepted_error.is_some())?
        .with_value(
            "terms_accepted_error",
            RenderValue::text(terms_accepted_error.unwrap_or_default()),
        )
}

fn storefront_field_error(form_state: Option<&StorefrontFormState>, field: &str) -> Option<String> {
    form_state.and_then(|state| state.field_errors.get(field).cloned())
}

fn form_errors_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    form_state
        .map(|state| {
            state
                .field_errors
                .iter()
                .map(|(field, message)| {
                    RenderModel::new()
                        .with_value("field", RenderValue::text(field.clone()))?
                        .with_value("message", RenderValue::text(message.clone()))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}
