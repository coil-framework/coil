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
use coil_core::IntegrationKind;
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
use coil_observability::{DependencyStatus, MetricReading};
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
                    fields.insert("content_kind".to_string(), "legacy_html".to_string());
                    fields.insert("block_count".to_string(), "1".to_string());
                    fields.insert("has_blocks".to_string(), "true".to_string());
                    fields.insert("show_in_navigation".to_string(), "true".to_string());
                    fields.insert("allow_indexing".to_string(), "true".to_string());
                    fields.insert("include_in_sitemap".to_string(), "true".to_string());
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
            .with_object(
                "navigation",
                navigation_model(Some(self), &execution.locale)?,
            )?
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

        apply_customer_render_model_contributions(
            self,
            execution,
            template_name,
            fragment_id,
            model,
        )
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
        coil_customer_sdk::MergePolicy::FailOnConflict => RenderModelMergePolicy::FailOnConflict,
        coil_customer_sdk::MergePolicy::ReplaceExisting => RenderModelMergePolicy::ReplaceExisting,
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

fn navigation_model(
    plan: Option<&RuntimePlan>,
    locale: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new().with_list("primary", primary_navigation_items(plan, locale)?)
}

fn nav_item(label: &str, href: &str) -> RenderModel {
    RenderModel::new()
        .with_value("label", RenderValue::text(label))
        .and_then(|model| model.with_value("href", RenderValue::text(href)))
        .expect("navigation item keys are valid")
}

fn normalize_navigation_href(href: &str, locale: &str) -> String {
    let locale = locale.trim_matches('/');
    let trimmed = href.trim();
    let normalized = if let Some((first, rest)) = trimmed.trim_start_matches('/').split_once('/') {
        if first.contains('-') && matches!(rest, "shop" | "shop/collections" | "events") {
            format!("/{rest}")
        } else {
            trimmed.to_string()
        }
    } else {
        trimmed.to_string()
    };

    match normalized.as_str() {
        "/shop" => format!("/{locale}/shop"),
        "/shop/collections" => format!("/{locale}/shop/collections"),
        "/events" => format!("/{locale}/events"),
        _ => normalized,
    }
}

fn primary_navigation_items(
    plan: Option<&RuntimePlan>,
    locale: &str,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let items = if let Some(plan) = plan {
        cms_admin_workspace(plan)?
            .navigation
            .into_iter()
            .map(|item| {
                nav_item(
                    item.label.as_str(),
                    normalize_navigation_href(item.href.as_str(), locale).as_str(),
                )
            })
            .collect::<Vec<_>>()
    } else {
        vec![
            nav_item("Home", "/"),
            nav_item(
                "Shop",
                format!("/{}/shop", locale.trim_matches('/')).as_str(),
            ),
            nav_item(
                "Collections",
                format!("/{}/shop/collections", locale.trim_matches('/')).as_str(),
            ),
            nav_item(
                "Events",
                format!("/{}/events", locale.trim_matches('/')).as_str(),
            ),
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
    let settings = cms_admin_workspace(plan)
        .map(|workspace| workspace.global_settings)
        .unwrap_or_else(|_| crate::cms_admin::default_workspace().global_settings);

    RenderModel::new()
        .with_value("id", RenderValue::text(site_id))?
        .with_value("display_name", RenderValue::text(display_name))?
        .with_value("request_host", RenderValue::text(execution.host.clone()))?
        .with_value("canonical_host", RenderValue::text(canonical_host))?
        .with_bool("has_brand_name", true)?
        .with_value("brand_name", RenderValue::text(brand_name))?
        .with_object("settings", cms_global_settings_model(&settings)?)
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
            "events",
            RenderValue::text(route_link(
                plan,
                site_id,
                "events.list",
                &BTreeMap::new(),
                Some(locale),
                &format!("/{locale}/events"),
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
        .with_value("passes", RenderValue::text("/account/passes"))?
        .with_value("admin_dashboard", RenderValue::text("/admin"))?
        .with_value("admin_customers", RenderValue::text("/admin/customers"))?
        .with_value("admin_diagnostics", RenderValue::text("/admin/diagnostics"))?
        .with_value("admin_jobs", RenderValue::text("/admin/jobs"))?
        .with_value(
            "admin_integrations",
            RenderValue::text("/admin/integrations"),
        )?
        .with_value("admin_audit", RenderValue::text("/admin/audit"))?
        .with_value("admin_orders", RenderValue::text("/admin/orders"))?
        .with_value("admin_payments", RenderValue::text("/admin/payments"))?
        .with_value("admin_search", RenderValue::text("/admin/search"))?
        .with_value("admin_reports", RenderValue::text("/admin/reports"))?
        .with_value("admin_bulk", RenderValue::text("/admin/bulk"))?
        .with_value("admin_recovery", RenderValue::text("/admin/recovery"))?
        .with_value("admin_events", RenderValue::text("/admin/events"))?
        .with_value(
            "admin_event_bookings",
            RenderValue::text("/admin/events/bookings"),
        )?
        .with_value(
            "admin_event_check_in",
            RenderValue::text("/admin/events/check-in"),
        )?
        .with_value(
            "admin_catalog",
            RenderValue::text("/admin/catalog/products"),
        )?
        .with_value("admin_pages", RenderValue::text("/admin/pages"))?
        .with_value(
            "admin_membership_tiers",
            RenderValue::text("/admin/memberships/tiers"),
        )?
        .with_value(
            "admin_membership_subscriptions",
            RenderValue::text("/admin/memberships/subscriptions"),
        )?
        .with_value(
            "admin_membership_passes",
            RenderValue::text("/admin/memberships/passes"),
        )?
        .with_value("admin_navigation", RenderValue::text("/admin/navigation"))?
        .with_value("admin_redirects", RenderValue::text("/admin/redirects"))?
        .with_value("admin_options", RenderValue::text("/admin/options"))
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
            let target_host =
                host_with_request_port(site.canonical_host.as_str(), execution.host.as_str());
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
                    locale_root_fallback_path(&plan.config, Some(site.id.as_str()), target_locale,)
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

fn route_page_title_and_summary(
    route_name: &str,
    params: &BTreeMap<String, String>,
    brand_name: &str,
) -> (String, &'static str) {
    let title = match route_name {
        "home" => brand_name.to_string(),
        "commerce.catalog" => format!("Shop {brand_name}"),
        "commerce.collections" => "Shop Collections".to_string(),
        "commerce.collection-detail" => params
            .get("collection_slug")
            .map(|slug| title_case_handle(slug))
            .unwrap_or_else(|| title_case_handle("featured")),
        "commerce.product-detail" => params
            .get("product_slug")
            .map(|slug| title_case_handle(slug))
            .unwrap_or_else(|| title_case_handle("harbor-cap")),
        "commerce.cart" => "Cart".to_string(),
        "commerce.checkout" => "Checkout".to_string(),
        "commerce.checkout-confirmation" => "Order Confirmed".to_string(),
        "commerce.account.orders" => "Order History".to_string(),
        "memberships.account.passes" => "Passes and Credits".to_string(),
        "admin.dashboard" => format!("{brand_name} Admin"),
        "admin.customers" => "Customer Operations".to_string(),
        "admin.diagnostics" => "Diagnostics".to_string(),
        "admin.jobs" => "Jobs".to_string(),
        "admin.integrations" => "Integrations".to_string(),
        "commerce.payment-operations" => "Payment Operations".to_string(),
        "ops.search" => "Search Operations".to_string(),
        "ops.reports" => "Reports".to_string(),
        "ops.bulk" => "Bulk Operations".to_string(),
        "ops.recovery" => "Recovery".to_string(),
        "events.admin.index" => "Event Operations".to_string(),
        "events.admin.bookings" => "Event Bookings".to_string(),
        "events.admin.check-in" => "Event Check-In".to_string(),
        "memberships.tiers" => "Membership Tiers".to_string(),
        "memberships.subscriptions" => "Membership Subscriptions".to_string(),
        "memberships.passes" => "Passes and Credits".to_string(),
        "admin.audit" => "Audit Log".to_string(),
        "commerce.orders" => "Orders".to_string(),
        "commerce.order-detail" => params
            .get("order_id")
            .map(|order_id| format!("Order {order_id}"))
            .unwrap_or_else(|| "Order Detail".to_string()),
        "commerce.catalog-admin" => "Catalog Administration".to_string(),
        "cms.pages.index" => "Pages".to_string(),
        "cms.navigation.index" => "Navigation".to_string(),
        "cms.redirects.index" => "Redirects".to_string(),
        "cms.options.index" => "Global Settings".to_string(),
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Your Account".to_string()
        }
        _ => route_name.to_string(),
    };

    let summary = match route_name {
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
        "memberships.account.passes" => {
            "Review event-linked passes, available credits, and the bookings they are intended to unlock."
        }
        "admin.dashboard" => {
            "Operator dashboard for launch-day catalog, orders, and content checks."
        }
        "admin.customers" => {
            "Operator-facing customer records built from live orders, memberships, and event-linked purchases."
        }
        "admin.diagnostics" => {
            "Readiness, metrics, trace samples, and privileged diagnostics entry points for operators."
        }
        "admin.jobs" => {
            "Registered background jobs, triggers, queues, and retry envelopes available in the operator shell."
        }
        "admin.integrations" => {
            "Module integration surfaces, approved outbound endpoints, and extension-facing platform seams."
        }
        "commerce.payment-operations" => {
            "Provider handoff, webhook confirmation, and downstream follow-up for live checkout activity."
        }
        "ops.search" => {
            "Search index freshness, rebuild pressure, and projection drift across the operator-visible catalog."
        }
        "ops.reports" => {
            "Operational report exports, delivery state, and queued follow-up work for system operators."
        }
        "ops.bulk" => {
            "Capability-gated bulk workflows for search reindex and queued export coordination."
        }
        "ops.recovery" => {
            "Operator-run recovery workflows for full restore and derived-state rebuild scenarios."
        }
        "events.admin.index" => {
            "Operator overview of live events, slot pressure, and booking or waitlist actions."
        }
        "events.admin.bookings" => {
            "Operator booking queue for held reservations, confirmed attendees, and waitlist follow-up."
        }
        "events.admin.check-in" => {
            "Operator check-in lane for attendance readiness, reconciliation, and on-site follow-up."
        }
        "memberships.tiers" => {
            "Operator view of the checked-in membership offers, entitlements, and storefront availability."
        }
        "memberships.subscriptions" => {
            "Operator-facing subscription queue built from live membership orders and current customer state."
        }
        "memberships.passes" => {
            "Operator-facing pass and credit queue built from event-linked purchases, pending payment state, and customer follow-up signals."
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
        "cms.options.index" => {
            "Edit shared storefront content and contact settings used across public templates."
        }
        "memberships.account" | "memberships.account.dashboard" | "account.dashboard" => {
            "Membership state, recent orders, and next actions for the signed-in customer."
        }
        _ => "Server-rendered storefront and account surface.",
    };

    (title, summary)
}

fn page_model_for_route_name(
    route_name: &str,
    params: &BTreeMap<String, String>,
    brand_name: &str,
    template_name: &str,
    fragment_id: Option<&str>,
) -> RenderModel {
    let (title, summary) = route_page_title_and_summary(route_name, params, brand_name);
    let presentation = page_presentation_model(template_name, fragment_id)
        .expect("page presentation model keys are valid");
    let settings =
        default_page_settings_model().expect("default page settings model keys are valid");
    let content = route_page_content_model(title.as_str(), summary)
        .expect("route page content model keys are valid");
    let blocks = empty_page_blocks().expect("empty page blocks model is valid");

    RenderModel::new()
        .with_value("title", RenderValue::text(title))
        .and_then(|model| model.with_value("summary", RenderValue::text(summary)))
        .and_then(|model| model.with_value("template", RenderValue::text(template_name)))
        .and_then(|model| {
            model.with_value("fragment_mode", RenderValue::bool(fragment_id.is_some()))
        })
        .and_then(|model| model.with_object("presentation", presentation))
        .and_then(|model| model.with_object("settings", settings))
        .and_then(|model| model.with_object("content", content))
        .and_then(|model| model.with_list("blocks", blocks))
        .and_then(|model| model.with_bool("has_blocks", false))
        .and_then(|model| model.with_value("block_count", RenderValue::text("0")))
        .expect("page model keys are valid")
}

fn page_model_for_route(
    execution: &RequestExecution,
    template_name: &str,
    fragment_id: Option<&str>,
) -> RenderModel {
    page_model_for_route_name(
        execution.route.route_name.as_str(),
        &execution.route.params,
        execution.brand_name.as_deref().unwrap_or("Shoppr"),
        template_name,
        fragment_id,
    )
}

fn page_presentation_model(
    template_name: &str,
    fragment_id: Option<&str>,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("template", RenderValue::text(template_name))?
        .with_bool("fragment_mode", fragment_id.is_some())?
        .with_bool("has_fragment_id", fragment_id.is_some())?
        .with_value(
            "fragment_id",
            RenderValue::text(fragment_id.unwrap_or_default().to_string()),
        )
}

fn default_page_settings_model() -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_bool("has_navigation_label", false)?
        .with_value("navigation_label", RenderValue::text(String::new()))?
        .with_bool("has_layout_variant", false)?
        .with_value("layout_variant", RenderValue::text(String::new()))?
        .with_bool("show_in_navigation", true)?
        .with_bool("allow_indexing", true)?
        .with_bool("include_in_sitemap", true)
}

fn route_page_content_model(title: &str, summary: &str) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("summary", RenderValue::text(summary))?
        .with_bool("has_body_html", false)?
        .with_bool("has_blocks", false)?
        .with_value("block_count", RenderValue::text("0"))
}

fn page_content_model_from_legacy_html(
    title: &str,
    summary: &str,
    body_html: &str,
) -> Result<RenderModel, TemplateModelError> {
    let blocks = legacy_page_blocks(body_html)?;
    let block_count = blocks.len() as i64;
    RenderModel::new()
        .with_value("title", RenderValue::text(title))?
        .with_value("summary", RenderValue::text(summary))?
        .with_value("body_source", RenderValue::text(body_html.to_string()))?
        .with_value(
            "body_html",
            RenderValue::trusted_html(TrustedHtml::new(body_html.to_string())?),
        )?
        .with_bool("has_body_html", true)?
        .with_bool("has_blocks", block_count > 0)?
        .with_value("block_count", RenderValue::text(block_count.to_string()))?
        .with_list("blocks", blocks)
}

fn empty_page_blocks() -> Result<Vec<RenderModel>, TemplateModelError> {
    Ok(Vec::new())
}

fn legacy_page_blocks(body_html: &str) -> Result<Vec<RenderModel>, TemplateModelError> {
    Ok(vec![legacy_html_page_block_model("page-body", body_html)?])
}

fn legacy_html_page_block_model(
    instance_id: &str,
    body_html: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("id", RenderValue::text(instance_id.to_string()))?
        .with_value("type", RenderValue::text("legacy_html_body"))?
        .with_value("type_id", RenderValue::text("legacy_html_body"))?
        .with_value("kind", RenderValue::text("inline"))?
        .with_value("render_mode", RenderValue::text("legacy_html"))?
        .with_bool("is_shared", false)?
        .with_bool("has_html", true)?
        .with_value("body_source", RenderValue::text(body_html.to_string()))?
        .with_value(
            "html",
            RenderValue::trusted_html(TrustedHtml::new(body_html.to_string())?),
        )
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
    let audience = storefront_audience_bindings(plan, locale, session, principal)?;

    match route_name {
        "home" | "commerce.catalog" | "commerce.collections" => {
            let has_catalog_sections = !fixture.catalog_sections.is_empty();
            let has_discovery_hubs = !fixture.discovery_hubs.is_empty();
            let has_product_cards = !fixture.product_cards.is_empty();
            model = model
                .with_object("audience", audience.audience.clone())?
                .with_bool("has_catalog_sections", has_catalog_sections)?
                .with_bool("has_discovery_hubs", has_discovery_hubs)?
                .with_bool("has_product_cards", has_product_cards)?
                .with_list("catalog_sections", fixture.catalog_sections.clone())?
                .with_list("discovery_hubs", fixture.discovery_hubs.clone())?
                .with_list("product_cards", fixture.product_cards.clone())?;
        }
        "commerce.collection-detail" => {
            let slug = params
                .get("collection_slug")
                .map(String::as_str)
                .unwrap_or("featured");
            model = model.with_object("audience", audience.audience.clone())?;
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
            let is_membership_product = catalog
                .visible_product_for_site(site_id, slug)
                .is_some_and(|product| product.product_kind == "membership");
            let show_active_membership_product_notice =
                is_membership_product && audience.has_membership;
            let show_pending_membership_product_notice =
                is_membership_product && audience.has_pending_membership_order;
            let show_membership_purchase_actions =
                !is_membership_product || audience.needs_membership_purchase;
            if catalog.visible_product_for_site(site_id, slug).is_some() {
                let product_cards = fixture.related_product_cards_for_product(slug);
                model = model
                    .with_object("audience", audience.audience.clone())?
                    .with_bool("has_product", true)?
                    .with_object("product", fixture.product_for(slug))?
                    .with_bool(
                        "show_active_membership_product_notice",
                        show_active_membership_product_notice,
                    )?
                    .with_bool(
                        "show_pending_membership_product_notice",
                        show_pending_membership_product_notice,
                    )?
                    .with_bool(
                        "show_membership_purchase_actions",
                        show_membership_purchase_actions,
                    )?
                    .with_bool("has_product_cards", !product_cards.is_empty())?
                    .with_list("product_cards", product_cards)?;
            } else {
                model = model
                    .with_object("audience", audience.audience.clone())?
                    .with_bool("has_product", false)?
                    .with_bool("show_active_membership_product_notice", false)?
                    .with_bool("show_pending_membership_product_notice", false)?
                    .with_bool("show_membership_purchase_actions", false)?
                    .with_bool("has_product_cards", false)?
                    .with_list("product_cards", Vec::<RenderModel>::new())?
                    .with_value(
                        "missing_product_handle",
                        RenderValue::text(slug.to_string()),
                    )?;
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
        "events.list" => {
            let events = event_fixtures(locale, site_id, plan)
                .into_iter()
                .map(|event| event_model(&event))
                .collect::<Result<Vec<_>, _>>()?;
            model = model
                .with_object("audience", audience.audience.clone())?
                .with_bool("has_events", !events.is_empty())?
                .with_list("events", events)?;
        }
        "events.detail" => {
            let slug = params
                .get("event_slug")
                .map(String::as_str)
                .unwrap_or("spring-tasting");
            let event = event_fixtures(locale, site_id, plan)
                .into_iter()
                .find(|event| event.slug == slug);
            model = model.with_object("audience", audience.audience.clone())?;
            if let Some(event) = event {
                model = model
                    .with_bool("has_event", true)?
                    .with_object("event", event_model(&event)?)?;
            } else {
                model = model
                    .with_bool("has_event", false)?
                    .with_value("missing_event_slug", RenderValue::text(slug.to_string()))?;
            }
        }
        "commerce.account.orders"
        | "memberships.account"
        | "memberships.account.dashboard"
        | "memberships.account.passes"
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
                .with_list("event_bookings", account.event_bookings)?
                .with_list("pass_programs", account.pass_programs)?
                .with_object("pass_wallet", account.pass_wallet)?
                .with_object("membership_summary", account.membership_summary)?;
        }
        "admin.dashboard" => {
            let (customer_records, customer_stats) = customer_operations_from_storefront(plan)?;
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
            let event_count = event_fixtures(locale, site_id, plan).len().to_string();
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_admin_panels", true)?
                .with_list(
                    "admin_panels",
                    admin_panels(locale, &fixture, order_count, customer_records.len())?,
                )?
                .with_value(
                    "catalog_count",
                    RenderValue::text(fixture.product_cards.len().to_string()),
                )?
                .with_value("order_count", RenderValue::text(order_count.to_string()))?
                .with_value(
                    "customer_count",
                    RenderValue::text(customer_records.len().to_string()),
                )?
                .with_value("event_count", RenderValue::text(event_count))?
                .with_value("content_count", RenderValue::text(content_count))?;
            model = model.with_object("customer_stats", customer_stats)?;
        }
        "admin.customers" => {
            let (customer_records, customer_stats) = customer_operations_from_storefront(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_customer_records", !customer_records.is_empty())?
                .with_list("customer_records", customer_records)?
                .with_object("customer_stats", customer_stats)?
                .with_value(
                    "customers_empty_text",
                    RenderValue::text(
                        "Complete at least one checkout so operator-visible customer records can be assembled.",
                    ),
                )?;
        }
        "admin.diagnostics" => {
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_object("diagnostic_status", admin_diagnostic_status(plan)?)?
                .with_list("diagnostic_metrics", admin_diagnostic_metrics(plan)?)?
                .with_list("diagnostic_traces", admin_diagnostic_traces(plan)?)?;
        }
        "admin.jobs" => {
            let rows = admin_job_rows(plan)?;
            let subscription_rows = admin_job_subscription_rows(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_job_rows", !rows.is_empty())?
                .with_list("job_rows", rows)?
                .with_bool("has_job_subscription_rows", !subscription_rows.is_empty())?
                .with_list("job_subscription_rows", subscription_rows)?
                .with_object("job_stats", admin_job_stats(plan)?)?;
        }
        "admin.integrations" => {
            let integration_rows = admin_integration_rows(plan)?;
            let outbound_rows = admin_outbound_endpoint_rows(plan)?;
            let extension_rows = admin_extension_participant_rows(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_integration_rows", !integration_rows.is_empty())?
                .with_list("integration_rows", integration_rows)?
                .with_bool("has_outbound_endpoint_rows", !outbound_rows.is_empty())?
                .with_list("outbound_endpoint_rows", outbound_rows)?
                .with_bool("has_extension_participant_rows", !extension_rows.is_empty())?
                .with_list("extension_participant_rows", extension_rows)?
                .with_object("integration_stats", admin_integration_stats(plan)?)?;
        }
        "commerce.payment-operations" => {
            let (payment_rows, payment_stats) = payment_operations_from_storefront(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_payment_operations", !payment_rows.is_empty())?
                .with_list("payment_operations", payment_rows)?
                .with_object("payment_operation_stats", payment_stats)?
                .with_value(
                    "payment_operations_empty_text",
                    RenderValue::text(
                        "Complete at least one checkout so provider handoff and webhook reconciliation become visible to operators.",
                    ),
                )?;
        }
        "ops.search" => {
            let rows = ops_search_rows(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_search_rows", !rows.is_empty())?
                .with_list("search_rows", rows)?
                .with_object("search_stats", ops_search_stats(plan)?)?;
        }
        "ops.reports" => {
            let rows = ops_report_rows(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_report_rows", !rows.is_empty())?
                .with_list("report_rows", rows)?
                .with_object("report_stats", ops_report_stats(plan)?)?;
        }
        "ops.bulk" => {
            let rows = ops_bulk_rows(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_bulk_rows", !rows.is_empty())?
                .with_list("bulk_rows", rows)?
                .with_object("bulk_stats", ops_bulk_stats(plan)?)?;
        }
        "ops.recovery" => {
            let rows = ops_recovery_rows(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_recovery_rows", !rows.is_empty())?
                .with_list("recovery_rows", rows)?
                .with_object("recovery_stats", ops_recovery_stats(plan)?)?;
        }
        "events.admin.index" => {
            let events = event_admin_rows(locale, site_id, plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_event_admin_rows", !events.is_empty())?
                .with_list("event_admin_rows", events)?
                .with_object(
                    "event_admin_stats",
                    event_admin_stats(locale, site_id, plan)?,
                )?;
        }
        "events.admin.bookings" => {
            let bookings = event_booking_rows(locale, site_id, plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_event_booking_rows", !bookings.is_empty())?
                .with_list("event_booking_rows", bookings)?
                .with_object(
                    "event_booking_stats",
                    event_booking_stats(locale, site_id, plan)?,
                )?;
        }
        "events.admin.check-in" => {
            let check_in_rows = event_check_in_rows(locale, site_id, plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_event_check_in_rows", !check_in_rows.is_empty())?
                .with_list("event_check_in_rows", check_in_rows)?
                .with_object(
                    "event_check_in_stats",
                    event_check_in_stats(locale, site_id, plan)?,
                )?;
        }
        "memberships.tiers" => {
            let tiers = membership_tiers_from_catalog(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_membership_tiers", !tiers.is_empty())?
                .with_list("membership_tiers", tiers)?
                .with_value(
                    "membership_tiers_empty_text",
                    RenderValue::text(
                        "No membership-backed products are visible in the checked-in catalog yet.",
                    ),
                )?;
        }
        "memberships.subscriptions" => {
            let (subscriptions, stats) = membership_subscriptions_from_storefront(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_membership_subscriptions", !subscriptions.is_empty())?
                .with_list("membership_subscriptions", subscriptions)?
                .with_object("membership_subscription_stats", stats)?
                .with_value(
                    "membership_subscriptions_empty_text",
                    RenderValue::text(
                        "Complete a membership checkout so operator-visible subscription records can be assembled.",
                    ),
                )?;
        }
        "memberships.passes" => {
            let (passes, stats) = membership_pass_rows_from_storefront(plan)?;
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_bool("has_membership_pass_rows", !passes.is_empty())?
                .with_list("membership_pass_rows", passes)?
                .with_object("membership_pass_stats", stats)?
                .with_value(
                    "membership_pass_rows_empty_text",
                    RenderValue::text(
                        "Complete an event-pass checkout so operator-visible pass and credit records can be assembled.",
                    ),
                )?;
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
                    cms_admin_selected_page_model_with_form_state(
                        selected_page,
                        form_state,
                        &workspace.shared_blocks,
                    )?,
                )?
                .with_bool("has_shared_blocks", !workspace.shared_blocks.is_empty())?
                .with_list(
                    "shared_blocks",
                    cms_admin_shared_blocks_model(&workspace.shared_blocks)?,
                )?
                .with_object(
                    "shared_block_editor",
                    cms_admin_shared_block_editor_model(form_state)?,
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
                cms_admin_selected_page_model_with_form_state(
                    selected_page,
                    form_state,
                    &workspace.shared_blocks,
                )?,
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
        "cms.options.index" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            model = model
                .with_object("operator", operator_identity(principal, session)?)?
                .with_object(
                    "global_settings",
                    cms_global_settings_model(&workspace.global_settings)?,
                )?;
            model = merge_cms_options_form_feedback(model, form_state)?;
        }
        "cms.page" => {
            let workspace = plan
                .map(cms_admin_workspace)
                .transpose()?
                .unwrap_or_else(default_cms_admin_workspace);
            let slug = params.get("slug").map(String::as_str).unwrap_or_default();
            let requires_membership = workspace
                .live_page_by_slug(slug)
                .and_then(|page| page.live.as_ref())
                .is_some_and(|revision| revision.settings.page_type == "membership_guide");
            model = model
                .with_object("audience", audience.audience.clone())?
                .with_bool(
                    "show_membership_gated_content",
                    !requires_membership || audience.has_membership,
                )?
                .with_bool(
                    "show_membership_pending_gate",
                    requires_membership && audience.has_pending_membership_order,
                )?
                .with_bool(
                    "show_membership_teaser_gate",
                    requires_membership && audience.needs_membership_purchase,
                )?
                .with_object("cms_page", cms_live_page_model(&workspace, slug)?)?;
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
        .with_list(
            "line_items",
            confirmation_line_items_from_storefront(order)?,
        )
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
            model.with_value(
                "line_count",
                RenderValue::text(order.line_count.to_string()),
            )
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
        return Ok((Vec::new(), admin_order_stats(0, 0, 0, 0)?));
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let orders = store.admin_orders(50).map_err(template_store_error)?;
    if orders.is_empty() {
        Ok((Vec::new(), admin_order_stats(0, 0, 0, 0)?))
    } else {
        let pending = orders
            .iter()
            .filter(|order| order.status == "pending_payment")
            .count();
        let payment_follow_up = orders
            .iter()
            .filter(|order| order.status == "pending_payment")
            .count();
        let refunded = orders
            .iter()
            .filter(|order| order.status == "refunded")
            .count();
        let rows = orders
            .iter()
            .map(|order| admin_order_row_from_storefront(plan, order))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((
            rows,
            admin_order_stats(orders.len(), pending, refunded, payment_follow_up)?,
        ))
    }
}

fn payment_operations_from_storefront(
    plan: Option<&RuntimePlan>,
) -> Result<(Vec<RenderModel>, RenderModel), TemplateModelError> {
    let Some(plan) = plan else {
        return Ok((Vec::new(), payment_operation_stats(0, 0, 0, 0)?));
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let orders = store.admin_orders(50).map_err(template_store_error)?;
    if orders.is_empty() {
        return Ok((Vec::new(), payment_operation_stats(0, 0, 0, 0)?));
    }

    let awaiting_confirmation = orders
        .iter()
        .filter(|order| order.status == "pending_payment")
        .count();
    let captured = orders
        .iter()
        .filter(|order| matches!(order.status.as_str(), "paid" | "fulfilled"))
        .count();
    let refund_follow_up = orders
        .iter()
        .filter(|order| matches!(order.status.as_str(), "refunded" | "partially_refunded"))
        .count();
    let rows = orders
        .iter()
        .map(|order| payment_operation_row_from_order(plan, order))
        .collect::<Result<Vec<_>, _>>()?;

    Ok((
        rows,
        payment_operation_stats(
            orders.len(),
            awaiting_confirmation,
            captured,
            refund_follow_up,
        )?,
    ))
}

fn payment_operation_stats(
    total: usize,
    awaiting_confirmation: usize,
    captured: usize,
    refund_follow_up: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value(
            "awaiting_confirmation",
            RenderValue::text(awaiting_confirmation.to_string()),
        )?
        .with_value("captured", RenderValue::text(captured.to_string()))?
        .with_value(
            "refund_follow_up",
            RenderValue::text(refund_follow_up.to_string()),
        )
}

fn payment_operation_row_from_order(
    plan: &RuntimePlan,
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    let provider_label = payment_provider_label(Some(plan));
    let has_payment_reference = order.payment.reference.is_some();
    let provider_status_label = payment_status_label(&order.payment.status).to_string();
    let webhook_status_label = if !has_payment_reference {
        "No provider reference recorded".to_string()
    } else if order.status == "pending_payment" {
        format!("Awaiting signed {provider_label} webhook")
    } else if matches!(
        order.status.as_str(),
        "paid" | "fulfilled" | "partially_refunded" | "refunded"
    ) {
        "Provider callback reconciled".to_string()
    } else {
        "Provider state needs review".to_string()
    };
    let next_job_label = if order.status == "pending_payment" {
        "Hold customer messaging until the payment-provider webhook confirms capture.".to_string()
    } else if matches!(order.status.as_str(), "paid" | "fulfilled") {
        "Queue order confirmation and operational follow-up from the captured payment.".to_string()
    } else if matches!(order.status.as_str(), "refunded" | "partially_refunded") {
        "Run refund follow-up and audit export after reconciliation.".to_string()
    } else {
        "Review local state before scheduling another downstream action.".to_string()
    };
    let integration_note = if order.status == "pending_payment" {
        configured_payment_provider(Some(plan))
            .map(|provider| provider.pending_next_step())
            .unwrap_or_else(|| {
                "Payment confirmation is pending. The order will move forward after the provider callback arrives.".to_string()
            })
    } else if matches!(order.status.as_str(), "paid" | "fulfilled") {
        "The provider handoff has completed. Operators can move into fulfillment or customer follow-up.".to_string()
    } else if matches!(order.status.as_str(), "refunded" | "partially_refunded") {
        "This payment already has refund history. Keep provider reconciliation and customer comms aligned.".to_string()
    } else {
        "Use the order detail view to reconcile payment, refund, and customer state.".to_string()
    };

    RenderModel::new()
        .with_value("reference", RenderValue::text(order.order_id.clone()))?
        .with_value(
            "checkout_email",
            RenderValue::text(order.payment.checkout_email.clone().unwrap_or_default()),
        )?
        .with_bool("has_checkout_email", order.payment.checkout_email.is_some())?
        .with_value(
            "payment_reference",
            RenderValue::text(order.payment.reference.clone().unwrap_or_default()),
        )?
        .with_bool("has_payment_reference", has_payment_reference)?
        .with_value("provider_label", RenderValue::text(provider_label))?
        .with_value(
            "provider_status_label",
            RenderValue::text(provider_status_label),
        )?
        .with_value(
            "webhook_status_label",
            RenderValue::text(webhook_status_label),
        )?
        .with_value("integration_note", RenderValue::text(integration_note))?
        .with_value("next_job_label", RenderValue::text(next_job_label))?
        .with_value("total", RenderValue::text(order.total.clone()))?
        .with_bool("has_refund_history", !order.refunds.is_empty())?
        .with_value(
            "refund_history_label",
            RenderValue::text(if order.refunds.is_empty() {
                String::new()
            } else {
                format!("{} refund event(s) recorded", order.refunds.len())
            }),
        )?
        .with_value("session_id", RenderValue::text(order.session_id.clone()))?
        .with_value(
            "detail_href",
            RenderValue::text(format!("/admin/orders/{}", order.order_id)),
        )
}

fn ops_search_rows(plan: Option<&RuntimePlan>) -> Result<Vec<RenderModel>, TemplateModelError> {
    let product_count = if let Some(plan) = plan {
        StorefrontStateStore::open_for_plan(plan)
            .map_err(template_store_error)?
            .catalog()
            .map_err(template_store_error)?
            .products
            .len()
    } else {
        0
    };
    Ok(vec![
        RenderModel::new()
            .with_value("index_name", RenderValue::text("catalog.products"))?
            .with_value("freshness_label", RenderValue::text("Fresh"))?
            .with_value(
                "drift_summary",
                RenderValue::text(format!(
                    "{product_count} catalog products currently participate in the checked-in search projection."
                )),
            )?
            .with_value(
                "trigger_summary",
                RenderValue::text("Rebuilt when catalog or CMS publication events invalidate browse content."),
            )?
            .with_value("action_label", RenderValue::text("Rebuild only if browse drift is confirmed."))?,
        RenderModel::new()
            .with_value("index_name", RenderValue::text("content.pages"))?
            .with_value("freshness_label", RenderValue::text("Watching publication events"))?
            .with_value(
                "drift_summary",
                RenderValue::text("CMS publication events enqueue search refresh work through the ops module."),
            )?
            .with_value(
                "trigger_summary",
                RenderValue::text("Publication and redirect changes should keep search projections aligned."),
            )?
            .with_value("action_label", RenderValue::text("Use this lane before forcing a bulk reindex."))?,
    ])
}

fn ops_search_stats(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let total = ops_search_rows(plan)?.len();
    let healthy = 1usize;
    let watching = total.saturating_sub(healthy);
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("healthy", RenderValue::text(healthy.to_string()))?
        .with_value("watching", RenderValue::text(watching.to_string()))
}

fn ops_report_rows(plan: Option<&RuntimePlan>) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    let mut definitions = plan.module_report_definitions.clone();
    definitions.sort_by(|left, right| left.definition.title.cmp(&right.definition.title));

    definitions
        .into_iter()
        .map(|registered| {
            let definition = registered.definition;
            let scope_summary = definition.description.clone().unwrap_or_else(|| {
                format!(
                    "Export generated by the {} module for operator handoff and audit follow-up.",
                    registered.module
                )
            });
            let output_summary = format!(
                "{} report delivered via {} under `{}`.",
                display_report_format(definition.format),
                display_report_delivery_mode(definition.delivery_mode),
                definition.export_prefix
            );
            let action_label = match definition.id.as_str() {
                "report.ops.search-health" => {
                    "Queue export before launch checks, browse drift review, or cutover sign-off."
                }
                "report.ops.backup-readiness" => {
                    "Queue export before backup drills, recovery review, or operational handoff."
                }
                _ => "Queue export when operators need a signed artifact.",
            };

            RenderModel::new()
                .with_value("report_id", RenderValue::text(definition.id.to_string()))?
                .with_value("report_name", RenderValue::text(definition.title))?
                .with_value(
                    "delivery_mode",
                    RenderValue::text(display_report_delivery_mode(definition.delivery_mode)),
                )?
                .with_value(
                    "format_label",
                    RenderValue::text(display_report_format(definition.format)),
                )?
                .with_value("job_state_label", RenderValue::text("Ready to export"))?
                .with_value("scope_summary", RenderValue::text(scope_summary))?
                .with_value("output_summary", RenderValue::text(output_summary))?
                .with_value("action_label", RenderValue::text(action_label))
        })
        .collect()
}

fn ops_report_stats(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let Some(plan) = plan else {
        return RenderModel::new()
            .with_value("total", RenderValue::text("0"))?
            .with_value("ready", RenderValue::text("0"))?
            .with_value("signed_delivery", RenderValue::text("0"))?
            .with_value("queued", RenderValue::text("0"));
    };
    let total = plan.module_report_definitions.len();
    let ready = total;
    let queued = total;
    let signed_delivery = plan
        .module_report_definitions
        .iter()
        .filter(|registered| {
            matches!(
                registered.definition.delivery_mode,
                coil_core::ReportDeliveryMode::SignedUrl
            )
        })
        .count();
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("ready", RenderValue::text(ready.to_string()))?
        .with_value(
            "signed_delivery",
            RenderValue::text(signed_delivery.to_string()),
        )?
        .with_value("queued", RenderValue::text(queued.to_string()))
}

fn ops_bulk_rows(plan: Option<&RuntimePlan>) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    let mut definitions = plan.module_bulk_operations.clone();
    definitions.sort_by(|left, right| left.definition.title.cmp(&right.definition.title));

    definitions
        .into_iter()
        .map(|registered| {
            let definition = registered.definition;
            let recommended_target_count = match definition.id.as_str() {
                "bulk.search.reindex" => "3",
                "bulk.reports.export" => "2",
                _ => "10",
            };
            RenderModel::new()
                .with_value("operation_id", RenderValue::text(definition.id.to_string()))?
                .with_value("operation_name", RenderValue::text(definition.title))?
                .with_value(
                    "scope_label",
                    RenderValue::text(display_bulk_scope(definition.scope)),
                )?
                .with_value(
                    "kind_label",
                    RenderValue::text(display_bulk_kind(definition.kind)),
                )?
                .with_value(
                    "target_limit_label",
                    RenderValue::text(
                        definition
                            .max_items
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "unbounded".to_string()),
                    ),
                )?
                .with_value(
                    "recommended_target_count",
                    RenderValue::text(recommended_target_count),
                )?
                .with_value(
                    "scope_summary",
                    RenderValue::text(
                        definition.description.unwrap_or_else(|| {
                            format!(
                                "Bulk workflow from the {} module for operator-run follow-up.",
                                registered.module
                            )
                        }),
                    ),
                )?
                .with_value(
                    "action_label",
                    RenderValue::text(match definition.id.as_str() {
                        "bulk.search.reindex" => {
                            "Queue this only after diagnostics and search-health review confirm browse drift."
                        }
                        "bulk.reports.export" => {
                            "Use this when multiple operator-facing exports are needed for audit or cutover sign-off."
                        }
                        _ => "Queue this workflow only when the operator runbook calls for it.",
                    }),
                )
        })
        .collect()
}

fn ops_bulk_stats(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let Some(plan) = plan else {
        return RenderModel::new()
            .with_value("total", RenderValue::text("0"))?
            .with_value("search", RenderValue::text("0"))?
            .with_value("system", RenderValue::text("0"));
    };

    let total = plan.module_bulk_operations.len();
    let search = plan
        .module_bulk_operations
        .iter()
        .filter(|registered| {
            matches!(
                registered.definition.scope,
                coil_core::BulkOperationScope::Search
            )
        })
        .count();
    let system = plan
        .module_bulk_operations
        .iter()
        .filter(|registered| {
            matches!(
                registered.definition.scope,
                coil_core::BulkOperationScope::System
            )
        })
        .count();

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("search", RenderValue::text(search.to_string()))?
        .with_value("system", RenderValue::text(system.to_string()))
}

fn ops_recovery_rows(plan: Option<&RuntimePlan>) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    plan.ops_catalog
        .recovery
        .definitions()
        .iter()
        .cloned()
        .map(|definition| {
            let stage_summary = definition
                .default_stages
                .iter()
                .map(|stage| display_recovery_stage(*stage))
                .collect::<Vec<_>>()
                .join(" -> ");

            RenderModel::new()
                .with_value("workflow_id", RenderValue::text(definition.id.to_string()))?
                .with_value("workflow_name", RenderValue::text(definition.title))?
                .with_value(
                    "requires_local_only_sensitive_ack",
                    RenderValue::text(
                        definition.requires_local_only_sensitive_ack.to_string(),
                    ),
                )?
                .with_value(
                    "stage_count",
                    RenderValue::text(definition.default_stages.len().to_string()),
                )?
                .with_value("stage_summary", RenderValue::text(stage_summary))?
                .with_value(
                    "workflow_summary",
                    RenderValue::text(definition.description.unwrap_or_else(|| {
                        "Recovery workflow for restoring source-of-truth state and rebuilding disposable layers."
                            .to_string()
                    })),
                )?
                .with_value(
                    "action_label",
                    RenderValue::text(match definition.id.as_str() {
                        "recovery.customer-app.full-restore" => {
                            "Use after source-of-truth restore is ready and the operator has confirmed host-local sensitive data handling."
                        }
                        "recovery.customer-app.derived-state" => {
                            "Use when data is intact but caches, search projections, or assets need coordinated rebuild."
                        }
                        _ => "Use the documented operator recovery runbook before queuing this workflow.",
                    }),
                )
        })
        .collect()
}

fn ops_recovery_stats(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let Some(plan) = plan else {
        return RenderModel::new()
            .with_value("total", RenderValue::text("0"))?
            .with_value("host_local", RenderValue::text("0"))?
            .with_value("derived_only", RenderValue::text("0"));
    };

    let total = plan.ops_catalog.recovery.definitions().len();
    let host_local = plan
        .ops_catalog
        .recovery
        .definitions()
        .iter()
        .filter(|definition| definition.requires_local_only_sensitive_ack)
        .count();
    let derived_only = plan
        .ops_catalog
        .recovery
        .definitions()
        .iter()
        .filter(|definition| !definition.requires_local_only_sensitive_ack)
        .count();

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("host_local", RenderValue::text(host_local.to_string()))?
        .with_value("derived_only", RenderValue::text(derived_only.to_string()))
}

fn admin_job_rows(plan: Option<&RuntimePlan>) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    let mut jobs = plan.registered_runtime_jobs.clone();
    jobs.sort_by(|left, right| left.contract.name.cmp(&right.contract.name));

    jobs.into_iter()
        .map(|registered| {
            let retry_summary = match registered.retry_policy.dead_letter_queue.as_ref() {
                Some(queue) => format!(
                    "{} attempts, {}s backoff start, dead-letter to {}.",
                    registered.retry_policy.max_attempts,
                    registered.retry_policy.initial_delay.as_secs(),
                    queue.as_str(),
                ),
                None => format!(
                    "{} attempts, {}s backoff start, no dedicated dead-letter queue.",
                    registered.retry_policy.max_attempts,
                    registered.retry_policy.initial_delay.as_secs(),
                ),
            };

            RenderModel::new()
                .with_value("job_name", RenderValue::text(registered.contract.name))?
                .with_value("module_name", RenderValue::text(registered.module))?
                .with_value(
                    "trigger_label",
                    RenderValue::text(display_job_trigger_kind(registered.contract.trigger)),
                )?
                .with_value("queue_name", RenderValue::text(registered.queue.as_str()))?
                .with_value(
                    "idempotent_label",
                    RenderValue::text(if registered.contract.idempotent {
                        "Idempotent"
                    } else {
                        "Non-idempotent"
                    }),
                )?
                .with_value(
                    "description",
                    RenderValue::text(registered.contract.description),
                )?
                .with_value("retry_summary", RenderValue::text(retry_summary))
        })
        .collect()
}

fn admin_job_stats(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let Some(plan) = plan else {
        return RenderModel::new()
            .with_value("total", RenderValue::text("0"))?
            .with_value("operator", RenderValue::text("0"))?
            .with_value("scheduled", RenderValue::text("0"))?
            .with_value("queue_depth", RenderValue::text("unavailable"));
    };

    let total = plan.registered_runtime_jobs.len();
    let operator = plan
        .registered_runtime_jobs
        .iter()
        .filter(|registered| registered.contract.trigger == JobTriggerKind::Operator)
        .count();
    let scheduled = plan
        .registered_runtime_jobs
        .iter()
        .filter(|registered| registered.contract.trigger == JobTriggerKind::Scheduled)
        .count();
    let event_reactions = plan.registered_runtime_event_subscriptions.len();
    let queue_depth = match plan
        .observability
        .telemetry
        .metric_reading("coil.queue.depth")
    {
        Some(MetricReading::Gauge(value)) => value.to_string(),
        Some(MetricReading::Counter(value)) => value.to_string(),
        Some(MetricReading::Histogram(value)) => value.last.to_string(),
        None => "unavailable".to_string(),
    };

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("operator", RenderValue::text(operator.to_string()))?
        .with_value("scheduled", RenderValue::text(scheduled.to_string()))?
        .with_value(
            "event_reactions",
            RenderValue::text(event_reactions.to_string()),
        )?
        .with_value("queue_depth", RenderValue::text(queue_depth))
}

fn admin_job_subscription_rows(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    let mut subscriptions = plan.registered_runtime_event_subscriptions.clone();
    subscriptions.sort_by(|left, right| {
        left.event_type
            .as_str()
            .cmp(right.event_type.as_str())
            .then_with(|| left.module.cmp(&right.module))
    });

    subscriptions
        .into_iter()
        .map(|subscription| {
            let queue_summary = format!(
                "Reaction queue {} -> target queue {}.",
                subscription.reaction_queue.as_str(),
                subscription.target_queue.as_str(),
            );
            RenderModel::new()
                .with_value("module_name", RenderValue::text(subscription.module))?
                .with_value(
                    "event_type",
                    RenderValue::text(subscription.event_type.as_str()),
                )?
                .with_value("job_name", RenderValue::text(subscription.job_name))?
                .with_value(
                    "trigger_label",
                    RenderValue::text(display_job_trigger_kind(subscription.target_trigger)),
                )?
                .with_value("queue_summary", RenderValue::text(queue_summary))?
                .with_value("description", RenderValue::text(subscription.description))
        })
        .collect()
}

fn admin_integration_rows(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    for module in &plan.modules {
        for integration in &module.integration_points {
            rows.push((
                module.name.clone(),
                integration.kind,
                integration.surface.clone(),
                integration.description.clone(),
                integration_operator_note(integration.kind).to_string(),
            ));
        }
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.2.cmp(&right.2)));

    rows.into_iter()
        .map(|(module_name, kind, surface, description, operator_note)| {
            RenderModel::new()
                .with_value("module_name", RenderValue::text(module_name))?
                .with_value(
                    "kind_label",
                    RenderValue::text(display_integration_kind(kind)),
                )?
                .with_value("surface", RenderValue::text(surface))?
                .with_value("description", RenderValue::text(description))?
                .with_value("operator_note", RenderValue::text(operator_note))
        })
        .collect()
}

fn admin_outbound_endpoint_rows(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    plan.approved_outbound_http_endpoints
        .iter()
        .map(|(name, endpoint)| {
            RenderModel::new()
                .with_value("endpoint_name", RenderValue::text(name.clone()))?
                .with_value("endpoint_url", RenderValue::text(endpoint.as_str().to_string()))?
                .with_value(
                    "operator_note",
                    RenderValue::text(
                        "Approved outbound endpoint declared in the runtime plan and available to modules or customer code.",
                    ),
                )
        })
        .collect()
}

fn admin_integration_stats(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let Some(plan) = plan else {
        return RenderModel::new()
            .with_value("total", RenderValue::text("0"))?
            .with_value("modules", RenderValue::text("0"))?
            .with_value("outbound_endpoints", RenderValue::text("0"))?
            .with_value("extensions", RenderValue::text("0"));
    };

    let total = plan
        .modules
        .iter()
        .map(|module| module.integration_points.len())
        .sum::<usize>();
    let modules = plan
        .modules
        .iter()
        .filter(|module| !module.integration_points.is_empty())
        .count();
    let outbound_endpoints = plan.approved_outbound_http_endpoints.len();
    let extensions = plan.installed_extensions.len() + plan.linked_customer_plugins.len();

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("modules", RenderValue::text(modules.to_string()))?
        .with_value(
            "outbound_endpoints",
            RenderValue::text(outbound_endpoints.to_string()),
        )?
        .with_value("extensions", RenderValue::text(extensions.to_string()))
}

fn admin_extension_participant_rows(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();

    for extension in &plan.installed_extensions {
        rows.push(
            RenderModel::new()
                .with_value("participant_kind", RenderValue::text("WASM extension"))?
                .with_value(
                    "participant_name",
                    RenderValue::text(extension.display_name.clone()),
                )?
                .with_value(
                    "participant_id",
                    RenderValue::text(extension.extension_id.clone()),
                )?
                .with_value(
                    "surface_summary",
                    RenderValue::text(format!(
                        "{} handlers installed for customer app {}.",
                        extension.handler_count, extension.customer_app_id
                    )),
                )?
                .with_value(
                    "operator_note",
                    RenderValue::text(
                        "Extension handlers run through declared slots and the shared host capability model.",
                    ),
                )?,
        );
    }

    for plugin in &plan.linked_customer_plugins {
        let hook_summary = if plugin.registered_hooks.is_empty() {
            "No runtime hooks registered.".to_string()
        } else {
            plugin
                .registered_hooks
                .iter()
                .map(registered_hook_label)
                .collect::<Vec<_>>()
                .join(", ")
        };
        rows.push(
            RenderModel::new()
                .with_value("participant_kind", RenderValue::text("Linked plugin"))?
                .with_value(
                    "participant_name",
                    RenderValue::text(plugin.display_name.clone()),
                )?
                .with_value(
                    "participant_id",
                    RenderValue::text(plugin.plugin_id.clone()),
                )?
                .with_value(
                    "surface_summary",
                    RenderValue::text(format!(
                        "Version {} with hooks: {}.",
                        plugin.version, hook_summary
                    )),
                )?
                .with_value(
                    "operator_note",
                    RenderValue::text(
                        "Customer plugins participate through explicit hook registration rather than ambient runtime mutation.",
                    ),
                )?,
        );
    }

    Ok(rows)
}

fn admin_diagnostic_status(plan: Option<&RuntimePlan>) -> Result<RenderModel, TemplateModelError> {
    let Some(plan) = plan else {
        return RenderModel::new()
            .with_value("liveness", RenderValue::text("Unknown"))?
            .with_value("readiness", RenderValue::text("Unknown"))?
            .with_value("metrics_enabled", RenderValue::text("false"))?
            .with_value("trace_enabled", RenderValue::text("false"))?
            .with_value("health_href", RenderValue::text("/health"))?
            .with_value("readiness_href", RenderValue::text("/ready"))?
            .with_value("metrics_href", RenderValue::text("/metrics"))?
            .with_value("diagnostics_href", RenderValue::text("/diagnostics"));
    };

    RenderModel::new()
        .with_value(
            "liveness",
            RenderValue::text(dependency_status_label(
                plan.observability.liveness.overall_status(),
            )),
        )?
        .with_value(
            "readiness",
            RenderValue::text(dependency_status_label(
                plan.observability.readiness.overall_status(),
            )),
        )?
        .with_value(
            "metrics_enabled",
            RenderValue::text(plan.observability.telemetry.metrics_enabled.to_string()),
        )?
        .with_value(
            "trace_enabled",
            RenderValue::text(plan.observability.telemetry.trace.enabled.to_string()),
        )?
        .with_value("health_href", RenderValue::text("/health"))?
        .with_value("readiness_href", RenderValue::text("/ready"))?
        .with_value("metrics_href", RenderValue::text("/metrics"))?
        .with_value("diagnostics_href", RenderValue::text("/diagnostics"))
}

fn admin_diagnostic_metrics(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };
    let telemetry = &plan.observability.telemetry;
    vec![
        ("coil.http.requests.total", "HTTP requests"),
        ("coil.http.requests.in_flight", "In-flight requests"),
        ("coil.http.request.latency_ms", "Request latency"),
        ("coil.queue.depth", "Queue depth"),
        ("coil.runtime.jobs.ready", "Ready jobs"),
    ]
    .into_iter()
    .map(|(metric, label)| {
        let value = match telemetry.metric_reading(metric) {
            Some(MetricReading::Counter(value)) => value.to_string(),
            Some(MetricReading::Gauge(value)) => value.to_string(),
            Some(MetricReading::Histogram(value)) => {
                format!(
                    "samples={}, last={}, max={}",
                    value.samples, value.last, value.max
                )
            }
            None => "unavailable".to_string(),
        };
        RenderModel::new()
            .with_value("metric_name", RenderValue::text(metric))?
            .with_value("metric_label", RenderValue::text(label))?
            .with_value("metric_value", RenderValue::text(value))
    })
    .collect()
}

fn admin_diagnostic_traces(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };
    plan.observability
        .telemetry
        .recent_traces(5)
        .into_iter()
        .map(|trace| {
            let duration_ms = trace
                .fields
                .get("duration_ms")
                .cloned()
                .unwrap_or_else(|| "unavailable".to_string());
            RenderModel::new()
                .with_value("trace_id", RenderValue::text(trace.trace_id))?
                .with_value("span", RenderValue::text(trace.span))?
                .with_value("outcome", RenderValue::text(trace.outcome))?
                .with_value(
                    "recorded_at_unix_seconds",
                    RenderValue::text(trace.recorded_at_unix_seconds.to_string()),
                )?
                .with_value("duration_ms", RenderValue::text(duration_ms))
        })
        .collect()
}

fn dependency_status_label(status: DependencyStatus) -> &'static str {
    match status {
        DependencyStatus::Healthy => "Healthy",
        DependencyStatus::Degraded => "Degraded",
        DependencyStatus::Unhealthy => "Unhealthy",
        DependencyStatus::Unknown => "Unknown",
    }
}

fn customer_operations_from_storefront(
    plan: Option<&RuntimePlan>,
) -> Result<(Vec<RenderModel>, RenderModel), TemplateModelError> {
    let records = admin_customer_records_from_storefront(plan)?;
    if records.is_empty() {
        return Ok((Vec::new(), admin_customer_stats(0, 0, 0, 0)?));
    }

    let mut active_memberships = 0usize;
    let mut pending_memberships = 0usize;
    let mut payment_follow_up = 0usize;
    let total = records.len();
    let mut rows = Vec::with_capacity(total);

    for row in records {
        if row.has_active_membership {
            active_memberships += 1;
        }
        if row.has_pending_membership {
            pending_memberships += 1;
        }
        if row.needs_payment_follow_up {
            payment_follow_up += 1;
        }
        rows.push(row.into_model()?);
    }

    Ok((
        rows,
        admin_customer_stats(
            total,
            active_memberships,
            pending_memberships,
            payment_follow_up,
        )?,
    ))
}

fn admin_customer_records_from_storefront(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<AdminCustomerRecordView>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };
    let store = StorefrontStateStore::open_for_plan(plan).map_err(template_store_error)?;
    let orders = store.admin_orders(100).map_err(template_store_error)?;
    if orders.is_empty() {
        return Ok(Vec::new());
    }

    let mut groups: BTreeMap<String, Vec<&StorefrontOrderSnapshot>> = BTreeMap::new();
    for order in &orders {
        groups
            .entry(admin_customer_group_key(order))
            .or_default()
            .push(order);
    }

    let mut rows = groups
        .into_values()
        .map(|orders| admin_customer_record_from_orders(orders))
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by(|left, right| right.sort_key.cmp(&left.sort_key));
    Ok(rows)
}

fn admin_customer_group_key(order: &StorefrontOrderSnapshot) -> String {
    order
        .principal_id
        .clone()
        .or_else(|| order.payment.checkout_email.clone())
        .unwrap_or_else(|| order.session_id.clone())
}

struct AdminCustomerRecordView {
    sort_key: u64,
    display_name: String,
    customer_email: String,
    has_customer_email: bool,
    principal_id: String,
    has_principal_id: bool,
    session_id: String,
    membership_state_label: String,
    membership_summary: String,
    has_active_membership: bool,
    has_pending_membership: bool,
    pass_state_label: String,
    pass_summary: String,
    pass_titles: String,
    pass_count: usize,
    has_active_pass: bool,
    has_pending_pass: bool,
    support_state_label: String,
    needs_payment_follow_up: bool,
    has_refund_history: bool,
    refund_history_label: String,
    event_access_label: String,
    latest_order_reference: String,
    latest_order_status: String,
    latest_order_total: String,
    latest_order_href: String,
    order_count: usize,
}

impl AdminCustomerRecordView {
    fn into_model(self) -> Result<RenderModel, TemplateModelError> {
        RenderModel::new()
            .with_value("display_name", RenderValue::text(self.display_name))?
            .with_value("customer_email", RenderValue::text(self.customer_email))?
            .with_bool("has_customer_email", self.has_customer_email)?
            .with_value("principal_id", RenderValue::text(self.principal_id))?
            .with_bool("has_principal_id", self.has_principal_id)?
            .with_value("session_id", RenderValue::text(self.session_id))?
            .with_value(
                "membership_state_label",
                RenderValue::text(self.membership_state_label),
            )?
            .with_value(
                "membership_summary",
                RenderValue::text(self.membership_summary),
            )?
            .with_bool("has_active_membership", self.has_active_membership)?
            .with_bool("has_pending_membership", self.has_pending_membership)?
            .with_value("pass_state_label", RenderValue::text(self.pass_state_label))?
            .with_value("pass_summary", RenderValue::text(self.pass_summary))?
            .with_value("pass_titles", RenderValue::text(self.pass_titles))?
            .with_value("pass_count", RenderValue::text(self.pass_count.to_string()))?
            .with_bool("has_active_pass", self.has_active_pass)?
            .with_bool("has_pending_pass", self.has_pending_pass)?
            .with_value(
                "support_state_label",
                RenderValue::text(self.support_state_label),
            )?
            .with_bool("needs_payment_follow_up", self.needs_payment_follow_up)?
            .with_bool("has_refund_history", self.has_refund_history)?
            .with_value(
                "refund_history_label",
                RenderValue::text(self.refund_history_label),
            )?
            .with_value(
                "event_access_label",
                RenderValue::text(self.event_access_label),
            )?
            .with_value(
                "latest_order_reference",
                RenderValue::text(self.latest_order_reference),
            )?
            .with_value(
                "latest_order_status",
                RenderValue::text(self.latest_order_status),
            )?
            .with_value(
                "latest_order_total",
                RenderValue::text(self.latest_order_total),
            )?
            .with_value(
                "latest_order_href",
                RenderValue::text(self.latest_order_href),
            )?
            .with_value(
                "order_count",
                RenderValue::text(self.order_count.to_string()),
            )
    }
}

fn admin_customer_record_from_orders(
    mut orders: Vec<&StorefrontOrderSnapshot>,
) -> Result<AdminCustomerRecordView, TemplateModelError> {
    orders.sort_by(|left, right| {
        right
            .created_at_unix_seconds
            .cmp(&left.created_at_unix_seconds)
    });
    let latest = orders
        .first()
        .copied()
        .ok_or_else(|| TemplateModelError::MissingValue {
            key: "admin_customers.latest_order".to_string(),
        })?;

    let customer_email = orders
        .iter()
        .find_map(|order| order.payment.checkout_email.clone())
        .unwrap_or_default();
    let principal_id = orders
        .iter()
        .find_map(|order| order.principal_id.clone())
        .unwrap_or_default();
    let session_id = latest.session_id.clone();
    let display_name = if !principal_id.is_empty() {
        display_name_from_principal_id(&principal_id)
    } else if !customer_email.is_empty() {
        display_name_from_principal_id(&customer_email)
    } else {
        format!("Guest {}", latest.order_id)
    };

    let has_active_membership = orders.iter().any(|order| {
        matches!(order.status.as_str(), "paid" | "fulfilled")
            && order
                .lines
                .iter()
                .any(admin_customer_order_has_membership_line)
    });
    let has_pending_membership = !has_active_membership
        && orders.iter().any(|order| {
            order.status == "pending_payment"
                && order
                    .lines
                    .iter()
                    .any(admin_customer_order_has_membership_line)
        });
    let active_pass_count = customer_pass_quantity_for_statuses(&orders, &["paid", "fulfilled"]);
    let pending_pass_count = if active_pass_count == 0 {
        customer_pass_quantity_for_statuses(&orders, &["pending_payment"])
    } else {
        0
    };
    let pass_titles = customer_pass_titles(&orders);
    let has_active_pass = active_pass_count > 0;
    let has_pending_pass = !has_active_pass && pending_pass_count > 0;
    let has_event_purchase = orders.iter().any(|order| {
        order
            .lines
            .iter()
            .any(admin_customer_order_has_event_linked_line)
    });
    let has_refund_history = orders.iter().any(|order| !order.refunds.is_empty());
    let needs_payment_follow_up = orders.iter().any(|order| order.status == "pending_payment");
    let has_refunded_order = orders
        .iter()
        .any(|order| matches!(order.status.as_str(), "refunded" | "partially_refunded"));

    let membership_state_label = if has_active_membership {
        "Active membership"
    } else if has_pending_membership {
        "Pending activation"
    } else {
        "No membership"
    };
    let membership_summary = if has_active_membership {
        "The latest qualifying membership purchase is paid or fulfilled for this customer record."
    } else if has_pending_membership {
        "A qualifying membership purchase exists but payment capture has not settled yet."
    } else {
        "No qualifying membership purchase has been captured for this customer record yet."
    };
    let pass_state_label = if has_active_pass {
        "Passes available"
    } else if has_pending_pass {
        "Pending pass activation"
    } else {
        "No passes"
    };
    let pass_summary = if has_active_pass {
        format!(
            "{} available across {}.",
            unit_count_label(active_pass_count, "pass", "passes"),
            pass_titles.as_str()
        )
    } else if has_pending_pass {
        format!(
            "{} will become available after payment capture settles.",
            unit_count_label(pending_pass_count, "pass", "passes")
        )
    } else {
        "No pass-backed entitlement has been captured for this customer record yet.".to_string()
    };
    let support_state_label = if needs_payment_follow_up {
        "Needs payment follow-up"
    } else if has_refunded_order {
        "Has refund history"
    } else if latest.status == "fulfilled" {
        "Fulfilled"
    } else {
        "Active customer record"
    };
    let refund_history_label = if has_refund_history {
        "Refund history present"
    } else {
        ""
    };
    let event_access_label = if has_event_purchase {
        "Has event-linked purchase"
    } else {
        "No event-linked purchase"
    };

    Ok(AdminCustomerRecordView {
        sort_key: latest.created_at_unix_seconds,
        display_name,
        customer_email: customer_email.clone(),
        has_customer_email: !customer_email.is_empty(),
        principal_id: principal_id.clone(),
        has_principal_id: !principal_id.is_empty(),
        session_id,
        membership_state_label: membership_state_label.to_string(),
        membership_summary: membership_summary.to_string(),
        has_active_membership,
        has_pending_membership,
        pass_state_label: pass_state_label.to_string(),
        pass_summary,
        pass_titles,
        pass_count: active_pass_count.max(pending_pass_count),
        has_active_pass,
        has_pending_pass,
        support_state_label: support_state_label.to_string(),
        needs_payment_follow_up,
        has_refund_history,
        refund_history_label: refund_history_label.to_string(),
        event_access_label: event_access_label.to_string(),
        latest_order_reference: latest.order_id.clone(),
        latest_order_status: display_status_label(&latest.status).to_string(),
        latest_order_total: latest.total.clone(),
        latest_order_href: format!("/admin/orders/{}", latest.order_id),
        order_count: orders.len(),
    })
}

fn admin_customer_order_has_membership_line(line: &StorefrontOrderLine) -> bool {
    line.product_kind == "membership"
        || line
            .entitlement_key
            .as_deref()
            .is_some_and(|key| key.starts_with("membership."))
}

fn admin_customer_order_has_pass_line(line: &StorefrontOrderLine) -> bool {
    matches!(
        line.product_kind.as_str(),
        "event_pass" | "credit" | "gift_credit"
    ) || line
        .entitlement_key
        .as_deref()
        .is_some_and(|key| key.starts_with("pass.") || key.starts_with("credit."))
        || line.sku.contains("pass")
        || line.sku.contains("credit")
        || line.title.to_ascii_lowercase().contains("pass")
        || line.title.to_ascii_lowercase().contains("credit")
}

fn admin_customer_order_has_event_linked_line(line: &StorefrontOrderLine) -> bool {
    matches!(
        line.product_kind.as_str(),
        "event" | "event_pass" | "experience" | "membership_event"
    ) || admin_customer_order_has_pass_line(line)
        || line
            .entitlement_key
            .as_deref()
            .is_some_and(|key| key.starts_with("event.") || key.contains("event"))
        || line.title.to_ascii_lowercase().contains("event")
}

fn customer_pass_quantity_for_statuses(
    orders: &[&StorefrontOrderSnapshot],
    eligible_statuses: &[&str],
) -> usize {
    orders
        .iter()
        .filter(|order| {
            eligible_statuses
                .iter()
                .any(|status| order.status.as_str() == *status)
        })
        .flat_map(|order| order.lines.iter())
        .filter(|line| admin_customer_order_has_pass_line(line))
        .map(|line| line.quantity as usize)
        .sum()
}

fn customer_pass_titles(orders: &[&StorefrontOrderSnapshot]) -> String {
    let titles = orders
        .iter()
        .flat_map(|order| order.lines.iter())
        .filter(|line| admin_customer_order_has_pass_line(line))
        .map(|line| line.title.clone())
        .collect::<std::collections::BTreeSet<_>>();

    if titles.is_empty() {
        "no active pass products".to_string()
    } else {
        titles.into_iter().collect::<Vec<_>>().join(", ")
    }
}

fn unit_count_label(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}

fn membership_tiers_from_catalog(
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(plan) = plan else {
        return Ok(Vec::new());
    };
    let catalog = StorefrontStateStore::open_for_plan(plan)
        .map_err(template_store_error)?
        .catalog()
        .map_err(template_store_error)?;
    let mut rows = catalog
        .products
        .iter()
        .filter(|product| product.product_kind == "membership")
        .map(|product| {
            let site_count = product.site_ids.len();
            Ok((
                site_count,
                RenderModel::new()
                    .with_value("title", RenderValue::text(product.title.clone()))?
                    .with_value("sku", RenderValue::text(product.sku.clone()))?
                    .with_value("handle", RenderValue::text(product.handle.clone()))?
                    .with_value(
                        "price",
                        RenderValue::text(money_display_minor(
                            product.price_minor,
                            &product.currency,
                        )),
                    )?
                    .with_value(
                        "variant_title",
                        RenderValue::text(product.variant_title.clone()),
                    )?
                    .with_value(
                        "entitlement_key",
                        RenderValue::text(product.entitlement_key.clone().unwrap_or_default()),
                    )?
                    .with_bool("has_entitlement_key", product.entitlement_key.is_some())?
                    .with_value("summary", RenderValue::text(product.summary.clone()))?
                    .with_value("site_count", RenderValue::text(site_count.to_string()))?
                    .with_value(
                        "site_summary",
                        RenderValue::text(if site_count == 1 {
                            "Visible in 1 site".to_string()
                        } else {
                            format!("Visible in {site_count} sites")
                        }),
                    )?
                    .with_value("sites", RenderValue::text(product.site_ids.join(", ")))?,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(rows.into_iter().map(|(_, model)| model).collect())
}

fn membership_subscriptions_from_storefront(
    plan: Option<&RuntimePlan>,
) -> Result<(Vec<RenderModel>, RenderModel), TemplateModelError> {
    let records = admin_customer_records_from_storefront(plan)?;
    if records.is_empty() {
        return Ok((Vec::new(), membership_subscription_stats(0, 0, 0, 0)?));
    }

    let mut active = 0usize;
    let mut pending = 0usize;
    let mut follow_up = 0usize;
    let mut event_linked = 0usize;

    let subscriptions = records
        .into_iter()
        .filter(|record| record.has_active_membership || record.has_pending_membership)
        .map(|record| {
            if record.has_active_membership {
                active += 1;
            }
            if record.has_pending_membership {
                pending += 1;
            }
            if record.needs_payment_follow_up {
                follow_up += 1;
            }
            if record.event_access_label == "Has event-linked purchase" {
                event_linked += 1;
            }

            RenderModel::new()
                .with_value("display_name", RenderValue::text(record.display_name))?
                .with_value("customer_email", RenderValue::text(record.customer_email))?
                .with_bool("has_customer_email", record.has_customer_email)?
                .with_value("principal_id", RenderValue::text(record.principal_id))?
                .with_bool("has_principal_id", record.has_principal_id)?
                .with_value(
                    "membership_state_label",
                    RenderValue::text(record.membership_state_label),
                )?
                .with_value(
                    "membership_summary",
                    RenderValue::text(record.membership_summary),
                )?
                .with_bool("has_active_membership", record.has_active_membership)?
                .with_bool("has_pending_membership", record.has_pending_membership)?
                .with_bool("needs_payment_follow_up", record.needs_payment_follow_up)?
                .with_value(
                    "support_state_label",
                    RenderValue::text(record.support_state_label),
                )?
                .with_value(
                    "event_access_label",
                    RenderValue::text(record.event_access_label),
                )?
                .with_value(
                    "latest_order_reference",
                    RenderValue::text(record.latest_order_reference),
                )?
                .with_value(
                    "latest_order_status",
                    RenderValue::text(record.latest_order_status),
                )?
                .with_value(
                    "latest_order_total",
                    RenderValue::text(record.latest_order_total),
                )?
                .with_value(
                    "latest_order_href",
                    RenderValue::text(record.latest_order_href),
                )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let total = subscriptions.len();
    Ok((
        subscriptions,
        membership_subscription_stats(total, active, pending, follow_up + event_linked)?,
    ))
}

fn membership_pass_rows_from_storefront(
    plan: Option<&RuntimePlan>,
) -> Result<(Vec<RenderModel>, RenderModel), TemplateModelError> {
    let records = admin_customer_records_from_storefront(plan)?;
    if records.is_empty() {
        return Ok((Vec::new(), membership_pass_stats(0, 0, 0, 0)?));
    }

    let mut available = 0usize;
    let mut pending = 0usize;
    let mut follow_up = 0usize;

    let passes = records
        .into_iter()
        .filter(|record| record.has_active_pass || record.has_pending_pass)
        .map(|record| {
            if record.has_active_pass {
                available += 1;
            }
            if record.has_pending_pass {
                pending += 1;
            }
            if record.needs_payment_follow_up {
                follow_up += 1;
            }

            RenderModel::new()
                .with_value("display_name", RenderValue::text(record.display_name))?
                .with_value("customer_email", RenderValue::text(record.customer_email))?
                .with_bool("has_customer_email", record.has_customer_email)?
                .with_value("principal_id", RenderValue::text(record.principal_id))?
                .with_bool("has_principal_id", record.has_principal_id)?
                .with_value(
                    "pass_state_label",
                    RenderValue::text(record.pass_state_label),
                )?
                .with_value("pass_summary", RenderValue::text(record.pass_summary))?
                .with_value("pass_titles", RenderValue::text(record.pass_titles))?
                .with_value(
                    "pass_count",
                    RenderValue::text(record.pass_count.to_string()),
                )?
                .with_bool("has_active_pass", record.has_active_pass)?
                .with_bool("has_pending_pass", record.has_pending_pass)?
                .with_value(
                    "support_state_label",
                    RenderValue::text(record.support_state_label),
                )?
                .with_value(
                    "latest_order_reference",
                    RenderValue::text(record.latest_order_reference),
                )?
                .with_value(
                    "latest_order_status",
                    RenderValue::text(record.latest_order_status),
                )?
                .with_value(
                    "latest_order_total",
                    RenderValue::text(record.latest_order_total),
                )?
                .with_value(
                    "latest_order_href",
                    RenderValue::text(record.latest_order_href),
                )
        })
        .collect::<Result<Vec<_>, _>>()?;

    let total = passes.len();
    Ok((
        passes,
        membership_pass_stats(total, available, pending, follow_up)?,
    ))
}

fn membership_subscription_stats(
    total: usize,
    active: usize,
    pending: usize,
    follow_up: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("active", RenderValue::text(active.to_string()))?
        .with_value("pending", RenderValue::text(pending.to_string()))?
        .with_value("follow_up", RenderValue::text(follow_up.to_string()))
}

fn membership_pass_stats(
    total: usize,
    available: usize,
    pending: usize,
    follow_up: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("available", RenderValue::text(available.to_string()))?
        .with_value("pending", RenderValue::text(pending.to_string()))?
        .with_value("follow_up", RenderValue::text(follow_up.to_string()))
}

fn event_admin_rows(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    event_fixtures(locale, site_id, plan)
        .into_iter()
        .map(|event| {
            let slot_count = event.timeslots.len();
            let waitlist_count = event
                .timeslots
                .iter()
                .filter(|slot| slot.booking_status_label.contains("Waitlist"))
                .count();
            let operational_state_label = if waitlist_count > 0 {
                "Waitlist pressure"
            } else if slot_count > 1 {
                "Multi-slot event"
            } else {
                "Single-session event"
            };
            let next_action_label = if waitlist_count > 0 {
                "Review waitlist and promotion rules"
            } else if event.audience_label.contains("members") {
                "Confirm member priority windows"
            } else {
                "Review booking windows"
            };

            RenderModel::new()
                .with_value("title", RenderValue::text(event.title))?
                .with_value("eyebrow", RenderValue::text(event.eyebrow))?
                .with_value("venue_name", RenderValue::text(event.venue_name))?
                .with_value("venue_city", RenderValue::text(event.venue_city))?
                .with_value("venue_mode", RenderValue::text(event.venue_mode))?
                .with_value("day_label", RenderValue::text(event.day_label))?
                .with_value(
                    "time_range_label",
                    RenderValue::text(event.time_range_label),
                )?
                .with_value("audience_label", RenderValue::text(event.audience_label))?
                .with_value(
                    "availability_label",
                    RenderValue::text(event.availability_label),
                )?
                .with_value(
                    "operational_state_label",
                    RenderValue::text(operational_state_label),
                )?
                .with_value("next_action_label", RenderValue::text(next_action_label))?
                .with_value("slot_count", RenderValue::text(slot_count.to_string()))?
                .with_value(
                    "slot_summary",
                    RenderValue::text(if slot_count == 1 {
                        "1 active slot".to_string()
                    } else {
                        format!("{slot_count} active slots")
                    }),
                )?
                .with_value("preview_href", RenderValue::text(event.detail_href))?
                .with_value("bookings_href", RenderValue::text("/admin/events/bookings"))?
                .with_value("check_in_href", RenderValue::text("/admin/events/check-in"))
        })
        .collect()
}

fn event_admin_stats(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Result<RenderModel, TemplateModelError> {
    let events = event_fixtures(locale, site_id, plan);
    let total = events.len();
    let slot_total = events
        .iter()
        .map(|event| event.timeslots.len())
        .sum::<usize>();
    let waitlist_events = events
        .iter()
        .filter(|event| {
            event
                .timeslots
                .iter()
                .any(|slot| slot.booking_status_label.contains("Waitlist"))
        })
        .count();
    let appointment_events = events
        .iter()
        .filter(|event| event.venue_mode == "Appointment")
        .count();

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("slot_total", RenderValue::text(slot_total.to_string()))?
        .with_value(
            "waitlist_events",
            RenderValue::text(waitlist_events.to_string()),
        )?
        .with_value(
            "appointment_events",
            RenderValue::text(appointment_events.to_string()),
        )
}

fn event_booking_rows(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let booking_fixtures = vec![
        (
            "EVT-2001",
            "Morgan Rowe",
            "morgan@example.com",
            "Spring Tasting Evening",
            "Early tasting",
            "Reservation held",
            "Gold member priority",
            "Needs confirmation before hold expiry",
        ),
        (
            "EVT-2002",
            "Avery King",
            "avery@example.com",
            "Summer Gala Preview",
            "Evening salon",
            "Waitlisted",
            "Invited members",
            "Check promotion order before sending updates",
        ),
        (
            "EVT-2003",
            "Jordan Lee",
            "jordan@example.com",
            "Fit Clinic Appointments",
            "Morning appointment",
            "Confirmed booking",
            "Pass-backed access",
            "Ready for check-in lane",
        ),
    ];

    let preview_hrefs = event_fixtures(locale, site_id, plan)
        .into_iter()
        .map(|event| (event.title, event.detail_href))
        .collect::<BTreeMap<_, _>>();

    booking_fixtures
        .into_iter()
        .map(
            |(
                reference,
                customer_name,
                customer_email,
                event_title,
                slot_label,
                state,
                audience,
                note,
            )| {
                RenderModel::new()
                    .with_value("reference", RenderValue::text(reference))?
                    .with_value("customer_name", RenderValue::text(customer_name))?
                    .with_value("customer_email", RenderValue::text(customer_email))?
                    .with_value("event_title", RenderValue::text(event_title))?
                    .with_value("slot_label", RenderValue::text(slot_label))?
                    .with_value("booking_state_label", RenderValue::text(state))?
                    .with_value("audience_label", RenderValue::text(audience))?
                    .with_value("support_note", RenderValue::text(note))?
                    .with_value(
                        "event_preview_href",
                        RenderValue::text(
                            preview_hrefs
                                .get(event_title)
                                .cloned()
                                .unwrap_or_else(|| format!("/{}/events", locale.trim_matches('/'))),
                        ),
                    )?
                    .with_value("check_in_href", RenderValue::text("/admin/events/check-in"))
            },
        )
        .collect()
}

fn event_booking_stats(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Result<RenderModel, TemplateModelError> {
    let rows = event_booking_rows(locale, site_id, plan)?;
    let total = rows.len();
    let waitlisted = 1usize;
    let confirmed = 1usize;
    let held = total.saturating_sub(waitlisted + confirmed);

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("held", RenderValue::text(held.to_string()))?
        .with_value("confirmed", RenderValue::text(confirmed.to_string()))?
        .with_value("waitlisted", RenderValue::text(waitlisted.to_string()))
}

fn event_check_in_rows(
    _locale: &str,
    _site_id: Option<&str>,
    _plan: Option<&RuntimePlan>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    Ok(vec![
        RenderModel::new()
            .with_value("booking_reference", RenderValue::text("EVT-2003"))?
            .with_value("attendee_name", RenderValue::text("Jordan Lee"))?
            .with_value("event_title", RenderValue::text("Fit Clinic Appointments"))?
            .with_value("slot_label", RenderValue::text("Morning appointment"))?
            .with_value(
                "attendance_state_label",
                RenderValue::text("Ready to check in"),
            )?
            .with_value(
                "operator_note",
                RenderValue::text(
                    "Pass-backed attendee. Confirm stylist assignment before scanning in.",
                ),
            )?,
        RenderModel::new()
            .with_value("booking_reference", RenderValue::text("EVT-2004"))?
            .with_value("attendee_name", RenderValue::text("Taylor Quinn"))?
            .with_value("event_title", RenderValue::text("Spring Tasting Evening"))?
            .with_value("slot_label", RenderValue::text("Main salon tasting"))?
            .with_value(
                "attendance_state_label",
                RenderValue::text("Already checked in"),
            )?
            .with_value(
                "operator_note",
                RenderValue::text(
                    "Attendance captured from the operator lane. Use this row for reconciliation.",
                ),
            )?,
    ])
}

fn event_check_in_stats(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Result<RenderModel, TemplateModelError> {
    let total = event_check_in_rows(locale, site_id, plan)?.len();
    let ready = 1usize;
    let completed = 1usize;

    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("ready", RenderValue::text(ready.to_string()))?
        .with_value("completed", RenderValue::text(completed.to_string()))
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
    let payment_provider_label = payment_provider_label(Some(plan));
    let payment_provider_summary = payment_provider_summary(Some(plan));
    let needs_payment_follow_up = order.status == "pending_payment";
    let payment_next_step = if needs_payment_follow_up {
        configured_payment_provider(Some(plan))
            .map(|provider| provider.pending_next_step())
            .unwrap_or_else(|| {
                "Payment confirmation is pending. The order will move forward after the provider callback arrives.".to_string()
            })
    } else if order.status == "paid" {
        "Payment is captured. The next operator step is fulfillment or customer support follow-up."
            .to_string()
    } else if order.status == "fulfilled" {
        "Payment is captured and the order is fulfilled. Use this view for refund or audit follow-up."
            .to_string()
    } else {
        "Review provider status and local order state before taking another payment-side action."
            .to_string()
    };
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
        .with_value(
            "payment_provider_label",
            RenderValue::text(payment_provider_label),
        )?
        .with_value(
            "payment_provider_summary",
            RenderValue::text(payment_provider_summary),
        )?
        .with_bool("needs_payment_follow_up", needs_payment_follow_up)?
        .with_value("payment_next_step", RenderValue::text(payment_next_step))?
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
        Some(principal_id) => {
            coil_auth::DefaultSubject::entity(coil_auth::Entity::user(principal_id.to_string()))
        }
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
    plan: &RuntimePlan,
    order: &StorefrontOrderSnapshot,
) -> Result<RenderModel, TemplateModelError> {
    let needs_payment_follow_up = order.status == "pending_payment";
    let support_state_label = if needs_payment_follow_up {
        format!(
            "Awaiting {} confirmation",
            payment_provider_operator_name(Some(plan))
        )
    } else if !order.refunds.is_empty() {
        "Refund history present".to_string()
    } else {
        "No payment follow-up required".to_string()
    };
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
            "payment_reference",
            RenderValue::text(order.payment.reference.clone().unwrap_or_default()),
        )?
        .with_bool("has_payment_reference", order.payment.reference.is_some())?
        .with_bool("needs_payment_follow_up", needs_payment_follow_up)?
        .with_value(
            "support_state_label",
            RenderValue::text(support_state_label),
        )?
        .with_value(
            "detail_href",
            RenderValue::text(format!("/admin/orders/{}", order.order_id)),
        )
}

fn admin_order_stats(
    total: usize,
    pending: usize,
    refunded: usize,
    payment_follow_up: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value("pending", RenderValue::text(pending.to_string()))?
        .with_value("refunded", RenderValue::text(refunded.to_string()))?
        .with_value(
            "payment_follow_up",
            RenderValue::text(payment_follow_up.to_string()),
        )
}

fn admin_customer_stats(
    total: usize,
    active_memberships: usize,
    pending_memberships: usize,
    payment_follow_up: usize,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("total", RenderValue::text(total.to_string()))?
        .with_value(
            "active_memberships",
            RenderValue::text(active_memberships.to_string()),
        )?
        .with_value(
            "pending_memberships",
            RenderValue::text(pending_memberships.to_string()),
        )?
        .with_value(
            "payment_follow_up",
            RenderValue::text(payment_follow_up.to_string()),
        )
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
        "cms.pages.save-settings" => "cms.page.edit",
        "cms.pages.save-blocks" => "cms.page.edit",
        "cms.pages.duplicate-block" => "cms.page.edit",
        "cms.pages.schedule" => "cms.page.publish",
        "cms.pages.publish" | "cms.pages.unpublish" => "cms.page.publish",
        "cms.pages.rollback" => "cms.page.publish",
        "cms.navigation.save" => "cms.navigation.edit",
        "cms.redirects.save" => "cms.page.edit",
        "cms.shared-blocks.save" => "cms.page.edit",
        "cms.options.save" => "cms.page.edit",
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
        "cms.pages.save-settings" => "Save page settings",
        "cms.pages.save-blocks" => "Save page blocks",
        "cms.pages.duplicate-block" => "Duplicate page block",
        "cms.pages.schedule" => "Schedule page publication",
        "cms.pages.publish" => "Publish page",
        "cms.pages.unpublish" => "Unpublish page",
        "cms.pages.rollback" => "Rollback page",
        "cms.navigation.save" => "Save navigation",
        "cms.redirects.save" => "Save redirects",
        "cms.shared-blocks.save" => "Save shared block",
        "cms.options.save" => "Save global settings",
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
    customer_count: usize,
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
            "Customers",
            "Review customer operations",
            "/admin/customers",
            &format!(
                "{customer_count} operator-visible customer records are currently assembled from live orders.",
            ),
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
        content_page(
            "Passes and credits",
            "/account/passes",
            "account",
            "Pass-backed access, balances, and event-linked redemption guidance for signed-in customers.",
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
            let blocks = cms_admin_revision_blocks_models(&page.draft, &workspace.shared_blocks)?;
            let scheduled_publish_at = page
                .scheduled_publish_at
                .map(|value| value.to_string())
                .unwrap_or_default();
            let previous_live_title = page
                .previous_live
                .as_ref()
                .map(|revision| revision.title.clone())
                .unwrap_or_default();
            RenderModel::new()
                .with_value("id", RenderValue::text(page.id.clone()))?
                .with_value("title", RenderValue::text(page.draft.title.clone()))?
                .with_value("slug", RenderValue::text(page.draft.slug.clone()))?
                .with_value(
                    "workflow_status",
                    RenderValue::text(page.status().to_string().to_lowercase()),
                )?
                .with_value(
                    "status_label",
                    RenderValue::text(page.status_label().to_string()),
                )?
                .with_value("summary", RenderValue::text(page.draft.summary.clone()))?
                .with_bool("has_blocks", !blocks.is_empty())?
                .with_value("block_count", RenderValue::text(blocks.len().to_string()))?
                .with_bool("has_scheduled_publish", page.scheduled_publish_at.is_some())?
                .with_value(
                    "scheduled_publish_at",
                    RenderValue::text(scheduled_publish_at),
                )?
                .with_bool("can_rollback", page.has_rollback_target())?
                .with_bool("has_previous_live", page.previous_live.is_some())?
                .with_value(
                    "previous_live_title",
                    RenderValue::text(previous_live_title),
                )?
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

fn default_cms_admin_page_settings() -> crate::cms_admin::CmsAdminPageSettings {
    crate::cms_admin::CmsAdminPageSettings {
        page_type: "page".to_string(),
        template: None,
        seo_title: None,
        seo_description: None,
        options: crate::cms_admin::CmsAdminPageOptions {
            show_in_navigation: false,
            allow_indexing: true,
            localized: false,
        },
    }
}

fn cms_admin_page_settings_model(
    settings: &crate::cms_admin::CmsAdminPageSettings,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("page_type", RenderValue::text(settings.page_type.clone()))?
        .with_bool("has_template", settings.template.is_some())?
        .with_value(
            "template",
            RenderValue::text(settings.template.clone().unwrap_or_default()),
        )?
        .with_bool("has_seo_title", settings.seo_title.is_some())?
        .with_value(
            "seo_title",
            RenderValue::text(settings.seo_title.clone().unwrap_or_default()),
        )?
        .with_bool("has_seo_description", settings.seo_description.is_some())?
        .with_value(
            "seo_description",
            RenderValue::text(settings.seo_description.clone().unwrap_or_default()),
        )?
        .with_bool("show_in_navigation", settings.options.show_in_navigation)?
        .with_bool("allow_indexing", settings.options.allow_indexing)?
        .with_bool("localized", settings.options.localized)?
        .with_bool("has_navigation_label", false)?
        .with_value("navigation_label", RenderValue::text(String::new()))?
        .with_bool("has_layout_variant", false)?
        .with_value("layout_variant", RenderValue::text(String::new()))?
        .with_bool("include_in_sitemap", settings.options.allow_indexing)
}

fn cms_admin_shared_block_lookup<'a>(
    shared_blocks: &'a [crate::cms_admin::CmsAdminSharedBlock],
    shared_block_id: &str,
) -> Option<&'a crate::cms_admin::CmsAdminSharedBlock> {
    shared_blocks
        .iter()
        .find(|block| block.id == shared_block_id)
}

fn cms_optional_form_state_field(
    form_state: Option<&StorefrontFormState>,
    field: &str,
) -> Option<String> {
    form_state
        .and_then(|state| state.fields.get(field))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn cms_checkbox_form_state_field(form_state: Option<&StorefrontFormState>, field: &str) -> bool {
    form_state
        .and_then(|state| state.fields.get(field))
        .map(|value| matches!(value.as_str(), "true" | "yes" | "on" | "1"))
        .unwrap_or(false)
}

fn cms_admin_page_settings_from_form_state(
    form_state: Option<&StorefrontFormState>,
    fallback: &crate::cms_admin::CmsAdminPageSettings,
) -> crate::cms_admin::CmsAdminPageSettings {
    let has_settings_fields = form_state
        .map(|state| {
            state.fields.keys().any(|field| {
                field.starts_with("page_settings_") || field.starts_with("page_option_")
            })
        })
        .unwrap_or(false);
    if !has_settings_fields {
        return fallback.clone();
    }
    crate::cms_admin::CmsAdminPageSettings {
        page_type: cms_optional_form_state_field(form_state, "page_settings_page_type")
            .unwrap_or_else(|| fallback.page_type.clone()),
        template: cms_optional_form_state_field(form_state, "page_settings_template"),
        seo_title: cms_optional_form_state_field(form_state, "page_settings_seo_title"),
        seo_description: cms_optional_form_state_field(form_state, "page_settings_seo_description"),
        options: crate::cms_admin::CmsAdminPageOptions {
            show_in_navigation: cms_checkbox_form_state_field(
                form_state,
                "page_option_show_in_navigation",
            ),
            allow_indexing: form_state
                .and_then(|state| state.fields.get("page_option_allow_indexing"))
                .map(|_| cms_checkbox_form_state_field(form_state, "page_option_allow_indexing"))
                .unwrap_or(fallback.options.allow_indexing),
            localized: cms_checkbox_form_state_field(form_state, "page_option_localized"),
        },
    }
}

fn cms_admin_page_blocks_from_form_state(
    form_state: Option<&StorefrontFormState>,
) -> Option<Vec<crate::cms_admin::CmsAdminPageBlock>> {
    let fields = &form_state?.fields;
    if !fields.keys().any(|field| field.starts_with("block_kind_")) {
        return None;
    }
    let mut blocks = Vec::new();
    for index in 0.. {
        let kind = fields
            .get(&format!("block_kind_{index}"))?
            .trim()
            .to_string();
        if kind.is_empty() {
            break;
        }
        let id = fields
            .get(&format!("block_id_{index}"))
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let label = fields
            .get(&format!("block_label_{index}"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let enabled = fields
            .get(&format!("block_enabled_{index}"))
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(true);
        if matches!(kind.as_str(), "shared" | "shared_reference") {
            let shared_block_id = fields
                .get(&format!("block_shared_block_id_{index}"))
                .map(|value| value.trim().to_string())
                .unwrap_or_default();
            blocks.push(crate::cms_admin::CmsAdminPageBlock::SharedReference(
                crate::cms_admin::CmsAdminSharedBlockReference {
                    id,
                    shared_block_id,
                    label,
                    enabled,
                },
            ));
            continue;
        }
        let block_type = fields
            .get(&format!("block_type_{index}"))
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        let prefix = format!("block_field_{index}_");
        let mut block_fields = std::collections::BTreeMap::new();
        for (field, value) in fields {
            if let Some(key) = field.strip_prefix(prefix.as_str()) {
                block_fields.insert(key.to_string(), value.clone());
            }
        }
        blocks.push(crate::cms_admin::CmsAdminPageBlock::Instance(
            crate::cms_admin::CmsAdminBlockInstance {
                id,
                block_type,
                label,
                enabled,
                fields: block_fields,
            },
        ));
    }
    Some(blocks)
}

fn cms_admin_fields_model(
    fields: &std::collections::BTreeMap<String, String>,
) -> Result<RenderModel, TemplateModelError> {
    let mut model = RenderModel::new();
    for (key, value) in fields {
        model = model.with_value(key.as_str(), RenderValue::text(value.clone()))?;
    }
    Ok(model)
}

fn cms_admin_field_entries_model(
    fields: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    fields
        .iter()
        .map(|(key, value)| {
            RenderModel::new()
                .with_value("key", RenderValue::text(key.clone()))?
                .with_value("value", RenderValue::text(value.clone()))
        })
        .collect()
}

fn cms_admin_named_field_entries_model(
    fields: &std::collections::BTreeMap<String, String>,
    prefix: &str,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    fields
        .iter()
        .map(|(key, value)| {
            RenderModel::new()
                .with_value("key", RenderValue::text(key.clone()))?
                .with_value("value", RenderValue::text(value.clone()))?
                .with_value("field_name", RenderValue::text(format!("{prefix}{key}")))
        })
        .collect()
}

fn empty_trusted_html_placeholder() -> &'static str {
    "<span hidden></span>"
}

fn cms_admin_page_block_model(
    block: &crate::cms_admin::CmsAdminPageBlock,
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
    index: usize,
) -> Result<RenderModel, TemplateModelError> {
    match block {
        crate::cms_admin::CmsAdminPageBlock::Instance(instance) => RenderModel::new()
            .with_value("id", RenderValue::text(instance.id.clone()))?
            .with_value("type", RenderValue::text(instance.block_type.clone()))?
            .with_value("type_id", RenderValue::text(instance.block_type.clone()))?
            .with_value("kind", RenderValue::text("inline"))?
            .with_value(
                "kind_field",
                RenderValue::text(format!("block_kind_{index}")),
            )?
            .with_value("id_field", RenderValue::text(format!("block_id_{index}")))?
            .with_value(
                "type_field",
                RenderValue::text(format!("block_type_{index}")),
            )?
            .with_value(
                "label_field",
                RenderValue::text(format!("block_label_{index}")),
            )?
            .with_bool("has_shared_block_id", false)?
            .with_value("shared_block_id", RenderValue::text(String::new()))?
            .with_value(
                "shared_block_id_field",
                RenderValue::text(format!("block_shared_block_id_{index}")),
            )?
            .with_value("render_mode", RenderValue::text("structured_fields"))?
            .with_bool("is_shared", false)?
            .with_bool("enabled", instance.enabled)?
            .with_bool("is_enabled", instance.enabled)?
            .with_bool("is_disabled", !instance.enabled)?
            .with_value(
                "enabled_field",
                RenderValue::text(format!("block_enabled_{index}")),
            )?
            .with_bool("has_label", instance.label.is_some())?
            .with_value(
                "label",
                RenderValue::text(instance.label.clone().unwrap_or_default()),
            )?
            .with_object("fields", cms_admin_fields_model(&instance.fields)?)?
            .with_list(
                "field_entries",
                cms_admin_named_field_entries_model(
                    &instance.fields,
                    format!("block_field_{index}_").as_str(),
                )?,
            )?
            .with_bool("has_html", instance.fields.contains_key("html"))
            .and_then(|model| {
                if let Some(html) = instance.fields.get("html") {
                    model.with_value(
                        "html",
                        RenderValue::trusted_html(TrustedHtml::new(html.clone())?),
                    )
                } else {
                    model.with_value(
                        "html",
                        RenderValue::trusted_html(TrustedHtml::new(
                            empty_trusted_html_placeholder(),
                        )?),
                    )
                }
            }),
        crate::cms_admin::CmsAdminPageBlock::SharedReference(reference) => {
            let shared = cms_admin_shared_block_lookup(shared_blocks, &reference.shared_block_id);
            let shared_type = shared
                .map(|block| block.block_type.clone())
                .unwrap_or_else(|| "shared_reference".to_string());
            let shared_label = shared
                .map(|block| block.label.clone())
                .unwrap_or_else(|| reference.shared_block_id.clone());
            let shared_fields = shared.map(|block| block.fields.clone()).unwrap_or_default();
            RenderModel::new()
                .with_value("id", RenderValue::text(reference.id.clone()))?
                .with_value("type", RenderValue::text(shared_type.clone()))?
                .with_value("type_id", RenderValue::text(shared_type))?
                .with_value("kind", RenderValue::text("shared_reference"))?
                .with_value(
                    "kind_field",
                    RenderValue::text(format!("block_kind_{index}")),
                )?
                .with_value("id_field", RenderValue::text(format!("block_id_{index}")))?
                .with_value(
                    "type_field",
                    RenderValue::text(format!("block_type_{index}")),
                )?
                .with_value(
                    "label_field",
                    RenderValue::text(format!("block_label_{index}")),
                )?
                .with_value("render_mode", RenderValue::text("shared_reference"))?
                .with_bool("is_shared", true)?
                .with_bool("enabled", reference.enabled)?
                .with_bool("is_enabled", reference.enabled)?
                .with_bool("is_disabled", !reference.enabled)?
                .with_value(
                    "enabled_field",
                    RenderValue::text(format!("block_enabled_{index}")),
                )?
                .with_bool("has_label", true)?
                .with_value(
                    "label",
                    RenderValue::text(
                        reference
                            .label
                            .clone()
                            .unwrap_or_else(|| shared_label.clone()),
                    ),
                )?
                .with_value(
                    "shared_block_id",
                    RenderValue::text(reference.shared_block_id.clone()),
                )?
                .with_bool("has_shared_block_id", true)?
                .with_value(
                    "shared_block_id_field",
                    RenderValue::text(format!("block_shared_block_id_{index}")),
                )?
                .with_bool("has_shared_block", shared.is_some())?
                .with_object("fields", cms_admin_fields_model(&shared_fields)?)?
                .with_list(
                    "field_entries",
                    cms_admin_field_entries_model(&shared_fields)?,
                )?
                .with_bool("has_html", shared_fields.contains_key("html"))
                .and_then(|model| {
                    if let Some(html) = shared_fields.get("html") {
                        model.with_value(
                            "html",
                            RenderValue::trusted_html(TrustedHtml::new(html.clone())?),
                        )
                    } else {
                        model.with_value(
                            "html",
                            RenderValue::trusted_html(TrustedHtml::new(
                                empty_trusted_html_placeholder(),
                            )?),
                        )
                    }
                })
        }
    }
}

fn cms_live_page_block_model(
    block: &crate::cms_admin::CmsAdminPageBlock,
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
) -> Result<RenderModel, TemplateModelError> {
    match block {
        crate::cms_admin::CmsAdminPageBlock::Instance(instance) => RenderModel::new()
            .with_value("id", RenderValue::text(instance.id.clone()))?
            .with_value("type", RenderValue::text(instance.block_type.clone()))?
            .with_value("type_id", RenderValue::text(instance.block_type.clone()))?
            .with_value("kind", RenderValue::text("inline"))?
            .with_value("source_kind", RenderValue::text("inline"))?
            .with_bool("is_shared", false)?
            .with_bool("enabled", instance.enabled)?
            .with_bool("is_enabled", instance.enabled)?
            .with_bool("is_disabled", !instance.enabled)?
            .with_bool("has_shared_block_id", false)?
            .with_bool("has_shared_block", false)?
            .with_value("shared_block_id", RenderValue::text(String::new()))?
            .with_value("shared_block_label", RenderValue::text(String::new()))?
            .with_bool("has_label", instance.label.is_some())?
            .with_value(
                "label",
                RenderValue::text(instance.label.clone().unwrap_or_default()),
            )?
            .with_object("fields", cms_admin_fields_model(&instance.fields)?)?
            .with_list(
                "field_entries",
                cms_admin_field_entries_model(&instance.fields)?,
            )?
            .with_bool("has_html", instance.fields.contains_key("html"))
            .and_then(|model| {
                if let Some(html) = instance.fields.get("html") {
                    model.with_value(
                        "html",
                        RenderValue::trusted_html(TrustedHtml::new(html.clone())?),
                    )
                } else {
                    model.with_value(
                        "html",
                        RenderValue::trusted_html(TrustedHtml::new(
                            empty_trusted_html_placeholder(),
                        )?),
                    )
                }
            }),
        crate::cms_admin::CmsAdminPageBlock::SharedReference(reference) => {
            let shared = cms_admin_shared_block_lookup(shared_blocks, &reference.shared_block_id);
            let shared_type = shared
                .map(|block| block.block_type.clone())
                .unwrap_or_else(|| "shared_reference".to_string());
            let shared_label = shared
                .map(|block| block.label.clone())
                .unwrap_or_else(|| reference.shared_block_id.clone());
            let shared_fields = shared.map(|block| block.fields.clone()).unwrap_or_default();
            RenderModel::new()
                .with_value("id", RenderValue::text(reference.id.clone()))?
                .with_value("type", RenderValue::text(shared_type.clone()))?
                .with_value("type_id", RenderValue::text(shared_type))?
                .with_value("kind", RenderValue::text("shared_reference"))?
                .with_value("source_kind", RenderValue::text("shared"))?
                .with_bool("is_shared", true)?
                .with_bool("enabled", reference.enabled)?
                .with_bool("is_enabled", reference.enabled)?
                .with_bool("is_disabled", !reference.enabled)?
                .with_bool("has_label", true)?
                .with_value(
                    "label",
                    RenderValue::text(
                        reference
                            .label
                            .clone()
                            .unwrap_or_else(|| shared_label.clone()),
                    ),
                )?
                .with_value(
                    "shared_block_id",
                    RenderValue::text(reference.shared_block_id.clone()),
                )?
                .with_value("shared_block_label", RenderValue::text(shared_label))?
                .with_bool("has_shared_block_id", true)?
                .with_bool("has_shared_block", shared.is_some())?
                .with_object("fields", cms_admin_fields_model(&shared_fields)?)?
                .with_list(
                    "field_entries",
                    cms_admin_field_entries_model(&shared_fields)?,
                )?
                .with_bool("has_html", shared_fields.contains_key("html"))
                .and_then(|model| {
                    if let Some(html) = shared_fields.get("html") {
                        model.with_value(
                            "html",
                            RenderValue::trusted_html(TrustedHtml::new(html.clone())?),
                        )
                    } else {
                        model.with_value(
                            "html",
                            RenderValue::trusted_html(TrustedHtml::new(
                                empty_trusted_html_placeholder(),
                            )?),
                        )
                    }
                })
        }
    }
}

fn cms_admin_revision_blocks_models(
    revision: &crate::cms_admin::CmsAdminPageRevision,
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    if revision.blocks.is_empty() {
        return legacy_page_blocks(revision.body_html.as_str());
    }
    revision
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| cms_admin_page_block_model(block, shared_blocks, index))
        .collect()
}

fn cms_live_revision_blocks_models(
    revision: &crate::cms_admin::CmsAdminPageRevision,
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    if revision.blocks.is_empty() {
        return legacy_page_blocks(revision.body_html.as_str());
    }
    revision
        .blocks
        .iter()
        .filter(|block| match block {
            crate::cms_admin::CmsAdminPageBlock::Instance(instance) => instance.enabled,
            crate::cms_admin::CmsAdminPageBlock::SharedReference(reference) => reference.enabled,
        })
        .map(|block| cms_live_page_block_model(block, shared_blocks))
        .collect()
}

fn cms_admin_shared_blocks_model(
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
) -> Result<Vec<RenderModel>, TemplateModelError> {
    shared_blocks
        .iter()
        .map(|block| {
            RenderModel::new()
                .with_value("id", RenderValue::text(block.id.clone()))?
                .with_value("label", RenderValue::text(block.label.clone()))?
                .with_value("type_id", RenderValue::text(block.block_type.clone()))?
                .with_object("fields", cms_admin_fields_model(&block.fields)?)?
                .with_list(
                    "field_entries",
                    cms_admin_named_field_entries_model(&block.fields, "shared_block_field_")?,
                )?
                .with_value(
                    "updated_at_unix_seconds",
                    RenderValue::text(block.updated_at.to_string()),
                )
        })
        .collect()
}

fn cms_admin_shared_block_editor_model(
    form_state: Option<&StorefrontFormState>,
) -> Result<RenderModel, TemplateModelError> {
    let id = cms_optional_form_state_field(form_state, "shared_block_id").unwrap_or_default();
    let label = cms_optional_form_state_field(form_state, "shared_block_label").unwrap_or_default();
    let block_type =
        cms_optional_form_state_field(form_state, "shared_block_type").unwrap_or_default();
    let mut fields = std::collections::BTreeMap::new();
    if let Some(state) = form_state {
        for (field, value) in &state.fields {
            if let Some(key) = field.strip_prefix("shared_block_field_") {
                fields.insert(key.to_string(), value.clone());
            }
        }
    }
    RenderModel::new()
        .with_value("id", RenderValue::text(id))?
        .with_value("label", RenderValue::text(label))?
        .with_value("type_id", RenderValue::text(block_type))?
        .with_object("fields", cms_admin_fields_model(&fields)?)?
        .with_list(
            "field_entries",
            cms_admin_named_field_entries_model(&fields, "shared_block_field_")?,
        )
}

fn cms_admin_selected_page_model(
    page: CmsAdminPage,
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
) -> Result<RenderModel, TemplateModelError> {
    let content = page_content_model_from_legacy_html(
        page.draft.title.as_str(),
        page.draft.summary.as_str(),
        page.draft.body_html.as_str(),
    )?;
    let blocks = cms_admin_revision_blocks_models(&page.draft, shared_blocks)?;
    RenderModel::new()
        .with_value("id", RenderValue::text(page.id.clone()))?
        .with_value("title", RenderValue::text(page.draft.title.clone()))?
        .with_value("slug", RenderValue::text(page.draft.slug.clone()))?
        .with_value("summary", RenderValue::text(page.draft.summary.clone()))?
        .with_value(
            "body_source",
            RenderValue::text(page.draft.body_html.clone()),
        )?
        .with_value(
            "body_html",
            RenderValue::trusted_html(TrustedHtml::new(page.draft.body_html.clone())?),
        )?
        .with_value(
            "status_label",
            RenderValue::text(page.status_label().to_string()),
        )?
        .with_object(
            "settings",
            cms_admin_page_settings_model(&page.draft.settings)?,
        )?
        .with_object("content", content)?
        .with_list("blocks", blocks)?
        .with_bool("has_blocks", !page.draft.blocks.is_empty())?
        .with_value(
            "block_count",
            RenderValue::text(page.draft.blocks.len().to_string()),
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
    shared_blocks: &[crate::cms_admin::CmsAdminSharedBlock],
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
    let scheduled_publish_at = page
        .as_ref()
        .and_then(|page| page.scheduled_publish_at)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let previous_live_title = page
        .as_ref()
        .and_then(|page| page.previous_live.as_ref())
        .map(|revision| revision.title.clone())
        .unwrap_or_default();
    let settings = page
        .as_ref()
        .map(|page| page.draft.settings.clone())
        .map(|settings| cms_admin_page_settings_from_form_state(form_state, &settings))
        .unwrap_or_else(|| {
            cms_admin_page_settings_from_form_state(form_state, &default_cms_admin_page_settings())
        });
    let blocks = if let Some(form_blocks) = cms_admin_page_blocks_from_form_state(form_state) {
        form_blocks
            .iter()
            .enumerate()
            .map(|(index, block)| cms_admin_page_block_model(block, shared_blocks, index))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        match page.as_ref() {
            Some(page) => cms_admin_revision_blocks_models(&page.draft, shared_blocks)?,
            None => legacy_page_blocks(&body_html)?,
        }
    };
    let content = page_content_model_from_legacy_html(&title, &summary, &body_html)?;
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
        .with_value(
            "workflow_status",
            RenderValue::text(
                page.as_ref()
                    .map(|page| page.status().to_string().to_lowercase())
                    .unwrap_or_else(|| "draft_only".to_string()),
            ),
        )?
        .with_value("status_label", RenderValue::text(status_label))?
        .with_object("settings", cms_admin_page_settings_model(&settings)?)?
        .with_object("content", content)?
        .with_list("blocks", blocks.clone())?
        .with_bool("has_blocks", !blocks.is_empty())?
        .with_value("block_count", RenderValue::text(blocks.len().to_string()))?
        .with_bool("has_live_path", has_live_path)?
        .with_value("live_path", RenderValue::text(live_path))?
        .with_bool(
            "has_scheduled_publish",
            page.as_ref()
                .and_then(|page| page.scheduled_publish_at)
                .is_some(),
        )?
        .with_value(
            "scheduled_publish_at",
            RenderValue::text(scheduled_publish_at),
        )?
        .with_bool(
            "can_rollback",
            page.as_ref().is_some_and(|page| page.has_rollback_target()),
        )?
        .with_bool(
            "has_previous_live",
            page.as_ref()
                .is_some_and(|page| page.previous_live.is_some()),
        )?
        .with_value(
            "previous_live_title",
            RenderValue::text(previous_live_title),
        )?
        .with_bool("has_preview_path", !preview_path.is_empty())?
        .with_value("preview_path", RenderValue::text(preview_path))
}

fn empty_cms_admin_selected_page_model() -> Result<RenderModel, TemplateModelError> {
    let placeholder_html = "<p>Create a draft page to preview it.</p>";
    let blocks = legacy_page_blocks(placeholder_html)?;
    RenderModel::new()
        .with_value("id", RenderValue::text(String::new()))?
        .with_value("title", RenderValue::text(String::new()))?
        .with_value("slug", RenderValue::text(String::new()))?
        .with_value("summary", RenderValue::text(String::new()))?
        .with_value("body_source", RenderValue::text(String::new()))?
        .with_value(
            "body_html",
            RenderValue::trusted_html(TrustedHtml::new(placeholder_html)?),
        )?
        .with_object(
            "settings",
            cms_admin_page_settings_model(&default_cms_admin_page_settings())?,
        )?
        .with_object(
            "content",
            page_content_model_from_legacy_html(
                String::new().as_str(),
                String::new().as_str(),
                placeholder_html,
            )?,
        )?
        .with_list("blocks", blocks.clone())?
        .with_bool("has_blocks", !blocks.is_empty())?
        .with_value("block_count", RenderValue::text(blocks.len().to_string()))?
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

fn cms_global_settings_model(
    settings: &crate::cms_admin::CmsAdminGlobalSettings,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value(
            "footer_heading",
            RenderValue::text(settings.footer_heading.clone()),
        )?
        .with_value(
            "footer_body",
            RenderValue::text(settings.footer_body.clone()),
        )?
        .with_value(
            "contact_email",
            RenderValue::text(settings.contact_email.clone()),
        )?
        .with_bool(
            "has_contact_email",
            !settings.contact_email.trim().is_empty(),
        )?
        .with_value(
            "contact_phone",
            RenderValue::text(settings.contact_phone.clone()),
        )?
        .with_bool(
            "has_contact_phone",
            !settings.contact_phone.trim().is_empty(),
        )?
        .with_value(
            "announcement_title",
            RenderValue::text(settings.announcement_title.clone()),
        )?
        .with_bool(
            "has_announcement_title",
            !settings.announcement_title.trim().is_empty(),
        )?
        .with_value(
            "announcement_body",
            RenderValue::text(settings.announcement_body.clone()),
        )?
        .with_bool(
            "has_announcement_body",
            !settings.announcement_body.trim().is_empty(),
        )?
        .with_value("footer_heading_field", RenderValue::text("footer_heading"))?
        .with_value("footer_body_field", RenderValue::text("footer_body"))?
        .with_value("contact_email_field", RenderValue::text("contact_email"))?
        .with_value("contact_phone_field", RenderValue::text("contact_phone"))?
        .with_value(
            "announcement_title_field",
            RenderValue::text("announcement_title"),
        )?
        .with_value(
            "announcement_body_field",
            RenderValue::text("announcement_body"),
        )
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
                .with_value(
                    "to_field",
                    RenderValue::text(format!("redirect_to_{index}")),
                )?
                .with_value(
                    "permanent_field",
                    RenderValue::text(format!("redirect_permanent_{index}")),
                )
        })
        .collect()
}

fn revision_has_distinct_structured_blocks(revision: &CmsAdminPageRevision) -> bool {
    if revision.blocks.is_empty() {
        return false;
    }
    if revision.blocks.len() == 1 {
        match &revision.blocks[0] {
            crate::cms_admin::CmsAdminPageBlock::Instance(instance)
                if instance.block_type == "rich_text" =>
            {
                return instance
                    .fields
                    .get("html")
                    .map(|html| html != &revision.body_html)
                    .unwrap_or(true);
            }
            crate::cms_admin::CmsAdminPageBlock::Instance(instance)
                if instance.block_type == "legacy_html_body" =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
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
        let has_structured_blocks = revision_has_distinct_structured_blocks(live);
        let content = page_content_model_from_legacy_html(
            live.title.as_str(),
            live.summary.as_str(),
            live.body_html.as_str(),
        )?;
        let blocks = cms_live_revision_blocks_models(live, &workspace.shared_blocks)?;
        return RenderModel::new()
            .with_bool("is_published", true)?
            .with_bool(
                "requires_membership",
                live.settings.page_type == "membership_guide",
            )?
            .with_value("title", RenderValue::text(live.title.clone()))?
            .with_value("summary", RenderValue::text(live.summary.clone()))?
            .with_value(
                "body_html",
                RenderValue::trusted_html(TrustedHtml::new(live.body_html.clone())?),
            )?
            .with_object("settings", cms_admin_page_settings_model(&live.settings)?)?
            .with_object("content", content)?
            .with_list("blocks", blocks.clone())?
            .with_bool("has_structured_blocks", has_structured_blocks)?
            .with_bool("has_blocks", !blocks.is_empty())?
            .with_value("block_count", RenderValue::text(blocks.len().to_string()))?
            .with_value("slug", RenderValue::text(live.slug.clone()));
    }

    RenderModel::new()
        .with_bool("is_published", false)?
        .with_bool("requires_membership", false)?
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
        .with_object(
            "settings",
            cms_admin_page_settings_model(&default_cms_admin_page_settings())?,
        )?
        .with_object(
            "content",
            route_page_content_model(
                "Page unavailable",
                "This CMS page is not published yet. Use the Shoppr admin workflow to publish it before linking customers to this path.",
            )?,
        )?
        .with_list("blocks", empty_page_blocks()?)?
        .with_bool("has_structured_blocks", false)?
        .with_bool("has_blocks", false)?
        .with_value("block_count", RenderValue::text("0"))?
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

fn merge_cms_options_form_feedback(
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
                    .unwrap_or_else(|| "Update the global settings and save again.".to_string()),
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
        .with_value(
            "provider_code",
            RenderValue::text(provider_code.to_string()),
        )?
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

fn payment_provider_operator_name(plan: Option<&RuntimePlan>) -> String {
    payment_provider_label(plan)
        .trim_end_matches(" webhook confirmation")
        .trim_end_matches(" hosted checkout")
        .trim_end_matches(" payment provider")
        .to_string()
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
    event_bookings: Vec<RenderModel>,
    pass_programs: Vec<RenderModel>,
    pass_wallet: RenderModel,
    membership_summary: RenderModel,
}

#[derive(Clone)]
struct AudienceSurfaceBindings {
    audience: RenderModel,
    has_membership: bool,
    has_pending_membership_order: bool,
    needs_membership_purchase: bool,
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
    let event_bookings = preview_event_bookings(locale, true, false)?;
    let has_event_bookings = !event_bookings.is_empty();
    let pass_programs = fixture_pass_programs(locale)?;
    let has_pass_programs = !pass_programs.is_empty();
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
            .with_bool("has_event_bookings", has_event_bookings)?
            .with_bool("has_pass_programs", has_pass_programs)?
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
            .with_value(
                "event_bookings_empty_text",
                RenderValue::text(
                    "Event reservations and confirmed bookings will appear here once the customer starts booking timed experiences.",
                ),
            )?
            .with_value(
                "pass_programs_empty_text",
                RenderValue::text(
                    "Pass-backed access and remaining credits will appear here once the customer completes an event-pass checkout.",
                ),
            )?
            .with_value("orders_cta_url", RenderValue::text(orders_cta_url))?
            .with_value("orders_cta_label", RenderValue::text(orders_cta_label))?
            .with_value(
                "membership_cta_url",
                RenderValue::text(localized_collection_path(locale, "memberships")),
            )?
            .with_value(
                "event_bookings_cta_url",
                RenderValue::text(format!("/{}/events", locale.trim_matches('/'))),
            )?
            .with_value(
                "event_bookings_cta_label",
                RenderValue::text("Browse event calendar"),
            )?
            .with_value(
                "passes_cta_url",
                RenderValue::text(localized_collection_path(locale, "events")),
            )?
            .with_value(
                "passes_cta_label",
                RenderValue::text("Browse event passes"),
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
        event_bookings,
        pass_programs,
        pass_wallet: pass_wallet_model(1, 0)?,
        membership_summary: fixture.membership_summary.clone(),
    })
}

fn storefront_audience_bindings(
    plan: Option<&RuntimePlan>,
    locale: &str,
    session: Option<&SessionContext>,
    principal: Option<&PrincipalContext>,
) -> Result<AudienceSurfaceBindings, TemplateModelError> {
    let snapshot = live_storefront_state(plan, session, principal)?;
    let active_membership = snapshot
        .as_ref()
        .map(|snapshot| projected_membership_state(snapshot, false))
        .transpose()?
        .flatten();
    let pending_membership = if active_membership.is_some() {
        None
    } else {
        snapshot
            .as_ref()
            .map(|snapshot| projected_membership_state(snapshot, true))
            .transpose()?
            .flatten()
    };
    let membership_tier_name = active_membership
        .as_ref()
        .or(pending_membership.as_ref())
        .map(|membership| membership.tier_name.clone())
        .unwrap_or_default();
    let has_membership = active_membership.is_some();
    let has_pending_membership_order = !has_membership && pending_membership.is_some();
    let needs_membership_purchase = !has_membership && !has_pending_membership_order;

    let (
        membership_state,
        membership_state_label,
        membership_title,
        membership_summary,
        membership_cta_label,
        membership_cta_url,
    ) = if let Some(active) = active_membership.as_ref() {
        (
            "active",
            active.status_label(),
            "Membership already active".to_string(),
            active.renewal_text.clone(),
            "Open memberships",
            "/account/memberships".to_string(),
        )
    } else if let Some(pending) = pending_membership.as_ref() {
        (
            "pending",
            pending.status_label(),
            "Membership activation pending".to_string(),
            pending.renewal_text.clone(),
            "Review order history",
            "/account/orders".to_string(),
        )
    } else {
        (
            "none",
            "Not active",
            "Membership preview".to_string(),
            "This surface previews member-specific guidance. Join from the storefront to unlock the full path in your account.".to_string(),
            "Explore memberships",
            localized_collection_path(locale, "memberships"),
        )
    };

    Ok(AudienceSurfaceBindings {
        audience: RenderModel::new()
            .with_bool("has_membership", has_membership)?
            .with_bool("has_pending_membership_order", has_pending_membership_order)?
            .with_bool("needs_membership_purchase", needs_membership_purchase)?
            .with_bool("has_membership_tier_name", !membership_tier_name.is_empty())?
            .with_value(
                "membership_tier_name",
                RenderValue::text(membership_tier_name),
            )?
            .with_value("membership_state", RenderValue::text(membership_state))?
            .with_value(
                "membership_state_label",
                RenderValue::text(membership_state_label),
            )?
            .with_value("membership_title", RenderValue::text(membership_title))?
            .with_value("membership_summary", RenderValue::text(membership_summary))?
            .with_value(
                "membership_cta_label",
                RenderValue::text(membership_cta_label),
            )?
            .with_value("membership_cta_url", RenderValue::text(membership_cta_url))?,
        has_membership,
        has_pending_membership_order,
        needs_membership_purchase,
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
    let event_bookings =
        preview_event_bookings(locale, has_membership, has_pending_membership_order)?;
    let has_event_bookings = !event_bookings.is_empty();
    let pass_programs = pass_programs_from_storefront(snapshot.as_ref(), locale)?;
    let pass_wallet = pass_wallet_from_storefront(snapshot.as_ref())?;
    let has_pass_programs = !pass_programs.is_empty();
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
            .with_bool("has_event_bookings", has_event_bookings)?
            .with_bool("has_pass_programs", has_pass_programs)?
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
            .with_value(
                "event_bookings_empty_text",
                RenderValue::text(
                    if principal_id.is_some() {
                        "No event reservations or confirmed bookings are attached to this signed-in account yet. Timed-event bookings will appear here after the live booking flow is completed."
                    } else {
                        "This browser session has no event reservations or bookings yet. Timed-event holds and confirmations will appear here after booking flows are completed from this same browser."
                    },
                ),
            )?
            .with_value(
                "pass_programs_empty_text",
                RenderValue::text(
                    if principal_id.is_some() {
                        "No pass-backed access is attached to this signed-in account yet. Complete an event-pass purchase to make the wallet and booking entitlements appear here."
                    } else {
                        "This browser session has no captured passes or credits yet. Event-pass purchases completed from this browser will appear here after payment capture."
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
                "event_bookings_cta_url",
                RenderValue::text(format!("/{}/events", locale.trim_matches('/'))),
            )?
            .with_value(
                "event_bookings_cta_label",
                RenderValue::text("Browse event calendar"),
            )?
            .with_value(
                "passes_cta_url",
                RenderValue::text(localized_collection_path(locale, "events")),
            )?
            .with_value(
                "passes_cta_label",
                RenderValue::text("Browse event passes"),
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
        event_bookings,
        pass_programs,
        pass_wallet,
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

fn fixture_pass_programs(locale: &str) -> Result<Vec<RenderModel>, TemplateModelError> {
    vec![pass_program_model(
        "Spring Tasting Pass",
        "tasting-pass",
        "Available",
        "1 available",
        "Use this pass when booking seasonal tasting slots from the event calendar.",
        localized_product_path(locale, "tasting-pass").as_str(),
        "/account/orders",
    )]
    .into_iter()
    .collect()
}

fn pass_programs_from_storefront(
    snapshot: Option<&StorefrontStateSnapshot>,
    locale: &str,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    let Some(snapshot) = snapshot else {
        return Ok(Vec::new());
    };

    let mut rows = Vec::new();
    for order in &snapshot.recent_orders {
        let state_label = match order.status.as_str() {
            "paid" | "fulfilled" => "Available",
            "pending_payment" => "Pending activation",
            "refunded" | "partially_refunded" => "Refunded",
            _ => "In review",
        };
        for line in &order.lines {
            if !admin_customer_order_has_pass_line(line) {
                continue;
            }
            let balance_label = if order.status == "pending_payment" {
                format!(
                    "{} pending",
                    unit_count_label(line.quantity as usize, "pass", "passes")
                )
            } else {
                format!(
                    "{} available",
                    unit_count_label(line.quantity as usize, "pass", "passes")
                )
            };
            rows.push(pass_program_model(
                &line.title,
                &line.sku,
                state_label,
                &balance_label,
                &pass_usage_summary(line),
                &localized_product_path(locale, &line.sku),
                "/account/orders",
            )?);
        }
    }

    Ok(rows)
}

fn pass_wallet_from_storefront(
    snapshot: Option<&StorefrontStateSnapshot>,
) -> Result<RenderModel, TemplateModelError> {
    let Some(snapshot) = snapshot else {
        return pass_wallet_model(0, 0);
    };

    let mut available = 0usize;
    let mut pending = 0usize;
    for order in &snapshot.recent_orders {
        for line in &order.lines {
            if !admin_customer_order_has_pass_line(line) {
                continue;
            }
            if matches!(order.status.as_str(), "paid" | "fulfilled") {
                available += line.quantity as usize;
            } else if order.status == "pending_payment" {
                pending += line.quantity as usize;
            }
        }
    }

    pass_wallet_model(available, pending)
}

fn pass_wallet_model(available: usize, pending: usize) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("available", RenderValue::text(available.to_string()))?
        .with_value("pending", RenderValue::text(pending.to_string()))?
        .with_bool("has_pending", pending > 0)?
        .with_value(
            "summary",
            RenderValue::text(if pending > 0 {
                format!(
                    "{} available, {} pending activation.",
                    unit_count_label(available, "pass", "passes"),
                    unit_count_label(pending, "pass", "passes")
                )
            } else {
                format!(
                    "{} currently available for event-linked bookings.",
                    unit_count_label(available, "pass", "passes")
                )
            }),
        )
}

fn pass_program_model(
    title: &str,
    sku: &str,
    state_label: &str,
    balance_label: &str,
    usage_summary: &str,
    product_href: &str,
    order_href: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("title", RenderValue::text(title.to_string()))?
        .with_value("sku", RenderValue::text(sku.to_string()))?
        .with_value("state_label", RenderValue::text(state_label.to_string()))?
        .with_value(
            "balance_label",
            RenderValue::text(balance_label.to_string()),
        )?
        .with_value(
            "usage_summary",
            RenderValue::text(usage_summary.to_string()),
        )?
        .with_value("product_href", RenderValue::text(product_href.to_string()))?
        .with_value("order_href", RenderValue::text(order_href.to_string()))
}

fn pass_usage_summary(line: &StorefrontOrderLine) -> String {
    if line.title.to_ascii_lowercase().contains("tasting") {
        "Use this pass when reserving tasting sessions or member-priority event slots.".to_string()
    } else if line.title.to_ascii_lowercase().contains("night") {
        "Use this pass for after-hours event inventory once the matching market window opens."
            .to_string()
    } else {
        "Use this pass when booking qualifying event inventory from the public calendar."
            .to_string()
    }
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

fn display_report_delivery_mode(mode: coil_core::ReportDeliveryMode) -> &'static str {
    match mode {
        coil_core::ReportDeliveryMode::PublicObjectStore => "Public object store",
        coil_core::ReportDeliveryMode::SignedUrl => "Signed URL",
        coil_core::ReportDeliveryMode::InternalOnly => "Internal only",
    }
}

fn display_job_trigger_kind(trigger: JobTriggerKind) -> &'static str {
    match trigger {
        JobTriggerKind::Scheduled => "Scheduled",
        JobTriggerKind::DomainEvent => "Domain event",
        JobTriggerKind::Operator => "Operator",
        JobTriggerKind::Webhook => "Webhook",
        JobTriggerKind::InlineFollowup => "Inline follow-up",
    }
}

fn integration_operator_note(kind: IntegrationKind) -> &'static str {
    match kind {
        IntegrationKind::AdminNavigation => "Extends the shared admin shell and operator routing.",
        IntegrationKind::AdminWorkflow => {
            "Participates in queueable or audited operator workflows."
        }
        IntegrationKind::FrontendRendering => {
            "Feeds the SSR and progressive-enhancement frontend contract."
        }
        IntegrationKind::SearchIndex => {
            "Contributes browse/search indexing behavior and rebuild pressure."
        }
        IntegrationKind::SeoMetadata | IntegrationKind::JsonLd => {
            "Shapes structured metadata emitted by storefront routes."
        }
        IntegrationKind::LocalizedContent => {
            "Participates in locale-aware editorial or storefront rendering."
        }
        IntegrationKind::CacheInvalidation => "Influences cache purge and freshness behavior.",
        IntegrationKind::StoragePolicy => {
            "Depends on the shared storage and artifact delivery policy."
        }
        IntegrationKind::CommerceBridge => {
            "Bridges storefront purchasing, fulfillment, or downstream commerce state."
        }
        IntegrationKind::AuthPublication => {
            "Coordinates auth-aware publication and access boundaries."
        }
    }
}

fn display_integration_kind(kind: IntegrationKind) -> &'static str {
    match kind {
        IntegrationKind::AdminNavigation => "Admin navigation",
        IntegrationKind::AdminWorkflow => "Admin workflow",
        IntegrationKind::FrontendRendering => "Frontend rendering",
        IntegrationKind::SearchIndex => "Search index",
        IntegrationKind::SeoMetadata => "SEO metadata",
        IntegrationKind::JsonLd => "JSON-LD",
        IntegrationKind::LocalizedContent => "Localized content",
        IntegrationKind::CacheInvalidation => "Cache invalidation",
        IntegrationKind::StoragePolicy => "Storage policy",
        IntegrationKind::CommerceBridge => "Commerce bridge",
        IntegrationKind::AuthPublication => "Auth publication",
    }
}

fn display_report_format(format: coil_core::ReportFormat) -> &'static str {
    match format {
        coil_core::ReportFormat::Csv => "CSV",
        coil_core::ReportFormat::Json => "JSON",
        coil_core::ReportFormat::Pdf => "PDF",
    }
}

fn display_bulk_kind(kind: coil_core::BulkOperationKind) -> &'static str {
    match kind {
        coil_core::BulkOperationKind::Publish => "Publish",
        coil_core::BulkOperationKind::Unpublish => "Unpublish",
        coil_core::BulkOperationKind::Reindex => "Reindex",
        coil_core::BulkOperationKind::Export => "Export",
        coil_core::BulkOperationKind::Cancel => "Cancel",
        coil_core::BulkOperationKind::CheckIn => "Check-in",
        coil_core::BulkOperationKind::Custom => "Custom",
    }
}

fn display_bulk_scope(scope: coil_core::BulkOperationScope) -> &'static str {
    match scope {
        coil_core::BulkOperationScope::Cms => "CMS",
        coil_core::BulkOperationScope::Commerce => "Commerce",
        coil_core::BulkOperationScope::Memberships => "Memberships",
        coil_core::BulkOperationScope::Events => "Events",
        coil_core::BulkOperationScope::Media => "Media",
        coil_core::BulkOperationScope::Search => "Search",
        coil_core::BulkOperationScope::System => "System",
    }
}

fn display_recovery_stage(stage: coil_ops::RecoveryStage) -> &'static str {
    match stage {
        coil_ops::RecoveryStage::RestoreDatabase => "Restore database",
        coil_ops::RecoveryStage::ReattachManagedObjectStore => "Reattach object store",
        coil_ops::RecoveryStage::RestoreLocalOnlySensitive => "Restore host-local sensitive data",
        coil_ops::RecoveryStage::RebuildDerivedState => "Rebuild derived state",
        coil_ops::RecoveryStage::RedeployStaticAssets => "Redeploy static assets",
        coil_ops::RecoveryStage::ValidateReadiness => "Validate readiness",
    }
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
    discovery_hubs: Vec<RenderModel>,
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
            product_kind: product.product_kind.clone(),
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
    let discovery_hubs = collections_data
        .iter()
        .map(|collection| discovery_hub_model(locale, collection, &products_data))
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
        .with_value(
            "payment_status_label",
            RenderValue::text("Ready for payment"),
        )?
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
        discovery_hubs,
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
                product_kind: product.product_kind.clone(),
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
    product_kind: String,
    collection_handle: String,
    collection_name: String,
}

struct EventSlotFixture {
    label: String,
    starts_at_label: String,
    ends_at_label: String,
    availability_label: String,
    audience_label: String,
    capacity_note: String,
    booking_status_label: String,
    booking_cta_label: String,
}

struct EventFixture {
    slug: String,
    title: String,
    summary: String,
    eyebrow: String,
    venue_name: String,
    venue_city: String,
    venue_mode: String,
    day_label: String,
    time_range_label: String,
    availability_label: String,
    audience_label: String,
    priority_note: String,
    detail_href: String,
    timeslots: Vec<EventSlotFixture>,
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

fn discovery_hub_model(
    locale: &str,
    collection: &CollectionFixture,
    products: &[ProductFixture],
) -> Result<RenderModel, TemplateModelError> {
    let product_count = products
        .iter()
        .filter(|product| {
            product.collection_handle == collection.handle || collection.handle == "featured"
        })
        .count();
    let (journey_title, journey_summary) = match collection.handle.as_str() {
        "memberships" => (
            "Membership-led discovery",
            "Start with recurring access products, then move into gated content and account state.",
        ),
        "events" => (
            "Event-led discovery",
            "Use event-linked products and passes to move from browse into time-bound experiences.",
        ),
        _ => (
            "Merchandising discovery",
            "Use curated edits to narrow the customer from the full catalog into a tighter browse path.",
        ),
    };

    RenderModel::new()
        .with_value("handle", RenderValue::text(collection.handle.as_str()))?
        .with_value("title", RenderValue::text(collection.title.as_str()))?
        .with_value("label", RenderValue::text(collection.label.as_str()))?
        .with_value("summary", RenderValue::text(collection.summary.as_str()))?
        .with_value("journey_title", RenderValue::text(journey_title))?
        .with_value("journey_summary", RenderValue::text(journey_summary))?
        .with_value(
            "href",
            RenderValue::text(localized_collection_path(
                locale,
                collection.handle.as_str(),
            )),
        )?
        .with_value(
            "product_count",
            RenderValue::text(product_count.to_string()),
        )
}

fn product_model(product: &ProductFixture) -> Result<RenderModel, TemplateModelError> {
    product_model_for_locale("en-GB", product, None)
}

fn event_slot_model(slot: &EventSlotFixture) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("label", RenderValue::text(slot.label.as_str()))?
        .with_value(
            "starts_at_label",
            RenderValue::text(slot.starts_at_label.as_str()),
        )?
        .with_value(
            "ends_at_label",
            RenderValue::text(slot.ends_at_label.as_str()),
        )?
        .with_value(
            "availability_label",
            RenderValue::text(slot.availability_label.as_str()),
        )?
        .with_value(
            "audience_label",
            RenderValue::text(slot.audience_label.as_str()),
        )?
        .with_value(
            "capacity_note",
            RenderValue::text(slot.capacity_note.as_str()),
        )?
        .with_value(
            "booking_status_label",
            RenderValue::text(slot.booking_status_label.as_str()),
        )?
        .with_value(
            "booking_cta_label",
            RenderValue::text(slot.booking_cta_label.as_str()),
        )
}

fn event_model(event: &EventFixture) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("slug", RenderValue::text(event.slug.as_str()))?
        .with_value("title", RenderValue::text(event.title.as_str()))?
        .with_value("summary", RenderValue::text(event.summary.as_str()))?
        .with_value("eyebrow", RenderValue::text(event.eyebrow.as_str()))?
        .with_value("venue_name", RenderValue::text(event.venue_name.as_str()))?
        .with_value("venue_city", RenderValue::text(event.venue_city.as_str()))?
        .with_value("venue_mode", RenderValue::text(event.venue_mode.as_str()))?
        .with_value("day_label", RenderValue::text(event.day_label.as_str()))?
        .with_value(
            "time_range_label",
            RenderValue::text(event.time_range_label.as_str()),
        )?
        .with_value(
            "availability_label",
            RenderValue::text(event.availability_label.as_str()),
        )?
        .with_value(
            "audience_label",
            RenderValue::text(event.audience_label.as_str()),
        )?
        .with_value(
            "priority_note",
            RenderValue::text(event.priority_note.as_str()),
        )?
        .with_value("href", RenderValue::text(event.detail_href.as_str()))?
        .with_bool("has_timeslots", !event.timeslots.is_empty())?
        .with_value(
            "timeslot_count",
            RenderValue::text(event.timeslots.len().to_string()),
        )?
        .with_list(
            "timeslots",
            event
                .timeslots
                .iter()
                .map(event_slot_model)
                .collect::<Result<Vec<_>, _>>()?,
        )
}

fn event_booking_model(
    id: &str,
    event_slug: &str,
    event_title: &str,
    slot_label: &str,
    status: &str,
    status_label: &str,
    day_label: &str,
    summary: &str,
    locale: &str,
) -> Result<RenderModel, TemplateModelError> {
    RenderModel::new()
        .with_value("id", RenderValue::text(id.to_string()))?
        .with_value("event_slug", RenderValue::text(event_slug.to_string()))?
        .with_value("event_title", RenderValue::text(event_title.to_string()))?
        .with_value("slot_label", RenderValue::text(slot_label.to_string()))?
        .with_value("status", RenderValue::text(status.to_string()))?
        .with_value("status_label", RenderValue::text(status_label.to_string()))?
        .with_value("day_label", RenderValue::text(day_label.to_string()))?
        .with_value("summary", RenderValue::text(summary.to_string()))?
        .with_value(
            "href",
            RenderValue::text(format!(
                "/{}/events/{}",
                locale.trim_matches('/'),
                event_slug.trim_matches('/')
            )),
        )
}

fn preview_event_bookings(
    locale: &str,
    has_membership: bool,
    has_pending_membership_order: bool,
) -> Result<Vec<RenderModel>, TemplateModelError> {
    if has_membership {
        return Ok(vec![
            event_booking_model(
                "booking-spring-tasting",
                "spring-tasting",
                "Spring Tasting Evening",
                "Early tasting",
                "confirmed",
                "Confirmed",
                "Thursday 11 April",
                "Seat confirmed. Arrive ten minutes early for the guided tasting.",
                locale,
            )?,
            event_booking_model(
                "booking-summer-gala",
                "summer-gala",
                "Summer Gala Preview",
                "Main salon waitlist",
                "waitlisted",
                "Waitlisted",
                "Saturday 6 July",
                "The primary salon is full, so this account is currently queued for the next available release.",
                locale,
            )?,
        ]);
    }

    if has_pending_membership_order {
        return Ok(vec![event_booking_model(
            "booking-fit-clinic",
            "fit-clinic",
            "Fit Clinic Appointments",
            "Personal fitting session",
            "reservation_held",
            "Reservation held",
            "Tuesday 7 May",
            "This provisional slot becomes a confirmed booking after the qualifying membership order settles.",
            locale,
        )?]);
    }

    Ok(Vec::new())
}

fn event_fixtures(
    locale: &str,
    site_id: Option<&str>,
    plan: Option<&RuntimePlan>,
) -> Vec<EventFixture> {
    let detail_href = |slug: &str| {
        let fallback = format!("/{locale}/events/{slug}");
        plan.map_or_else(
            || fallback.clone(),
            |plan| {
                route_link(
                    plan,
                    site_id,
                    "events.detail",
                    &BTreeMap::from([("event_slug".to_string(), slug.to_string())]),
                    Some(locale),
                    &fallback,
                )
            },
        )
    };

    vec![
        EventFixture {
            slug: "spring-tasting".to_string(),
            title: "Spring Tasting Evening".to_string(),
            summary:
                "A guided tasting and edit preview built around event-linked products and member-first booking."
                    .to_string(),
            eyebrow: "Member event".to_string(),
            venue_name: "Shoppr Townhouse".to_string(),
            venue_city: "London".to_string(),
            venue_mode: "In store".to_string(),
            day_label: "Thursday 11 April".to_string(),
            time_range_label: "18:30 to 20:30".to_string(),
            availability_label: "Priority booking window open".to_string(),
            audience_label: "Gold members book first".to_string(),
            priority_note:
                "Gold members can secure early slots before the wider event-linked edit opens."
                    .to_string(),
            detail_href: detail_href("spring-tasting"),
            timeslots: vec![
                EventSlotFixture {
                    label: "Early tasting".to_string(),
                    starts_at_label: "18:30".to_string(),
                    ends_at_label: "19:15".to_string(),
                    availability_label: "4 seats remaining".to_string(),
                    audience_label: "Gold members".to_string(),
                    capacity_note: "Small-group seating keeps this tasting intimate.".to_string(),
                    booking_status_label: "Priority reservation available".to_string(),
                    booking_cta_label: "Reserve seat".to_string(),
                },
                EventSlotFixture {
                    label: "Main salon tasting".to_string(),
                    starts_at_label: "19:30".to_string(),
                    ends_at_label: "20:30".to_string(),
                    availability_label: "8 seats remaining".to_string(),
                    audience_label: "All active members".to_string(),
                    capacity_note:
                        "The later session opens after the priority allocation clears."
                            .to_string(),
                    booking_status_label: "General reservation available".to_string(),
                    booking_cta_label: "Reserve seat".to_string(),
                },
            ],
        },
        EventFixture {
            slug: "summer-gala".to_string(),
            title: "Summer Gala Preview".to_string(),
            summary:
                "A late-evening campaign preview that combines styling, hospitality, and member access."
                    .to_string(),
            eyebrow: "Editorial event".to_string(),
            venue_name: "Riverside Studio".to_string(),
            venue_city: "Paris".to_string(),
            venue_mode: "In store".to_string(),
            day_label: "Friday 7 June".to_string(),
            time_range_label: "19:00 to 22:00".to_string(),
            availability_label: "Waitlist available".to_string(),
            audience_label: "Members and invited guests".to_string(),
            priority_note:
                "This event demonstrates that editorial positioning and booking operations belong in the same product story."
                    .to_string(),
            detail_href: detail_href("summer-gala"),
            timeslots: vec![EventSlotFixture {
                label: "Evening salon".to_string(),
                starts_at_label: "19:00".to_string(),
                ends_at_label: "22:00".to_string(),
                availability_label: "Join waitlist".to_string(),
                audience_label: "Invited members".to_string(),
                capacity_note:
                    "The main salon is full, so new interest routes through the waitlist path."
                        .to_string(),
                booking_status_label: "Waitlist only".to_string(),
                booking_cta_label: "Join waitlist".to_string(),
            }],
        },
        EventFixture {
            slug: "fit-clinic".to_string(),
            title: "Fit Clinic Appointments".to_string(),
            summary:
                "Short appointment-led sessions that show how timeslots and venue data can stay explicit."
                    .to_string(),
            eyebrow: "Service event".to_string(),
            venue_name: "Private Fitting Suite".to_string(),
            venue_city: "London".to_string(),
            venue_mode: "Appointment".to_string(),
            day_label: "Saturday 13 April".to_string(),
            time_range_label: "10:00 to 16:00".to_string(),
            availability_label: "Booking windows open".to_string(),
            audience_label: "Members and event-pass holders".to_string(),
            priority_note:
                "Appointment-led events use the same route model but make the timeslot layer more obvious."
                    .to_string(),
            detail_href: detail_href("fit-clinic"),
            timeslots: vec![
                EventSlotFixture {
                    label: "Morning appointment".to_string(),
                    starts_at_label: "10:00".to_string(),
                    ends_at_label: "10:45".to_string(),
                    availability_label: "2 appointments left".to_string(),
                    audience_label: "All active members".to_string(),
                    capacity_note: "One stylist is assigned to each appointment slot.".to_string(),
                    booking_status_label: "Appointments open".to_string(),
                    booking_cta_label: "Hold appointment".to_string(),
                },
                EventSlotFixture {
                    label: "Afternoon appointment".to_string(),
                    starts_at_label: "14:00".to_string(),
                    ends_at_label: "14:45".to_string(),
                    availability_label: "4 appointments left".to_string(),
                    audience_label: "Members and pass holders".to_string(),
                    capacity_note:
                        "Pass-backed slots open after membership-only windows have been offered."
                            .to_string(),
                    booking_status_label: "Appointments open".to_string(),
                    booking_cta_label: "Hold appointment".to_string(),
                },
            ],
        },
    ]
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
            "product_kind",
            RenderValue::text(product.product_kind.as_str()),
        )?
        .with_bool(
            "is_membership_product",
            product.product_kind == "membership",
        )?
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
    use crate::cms_admin::{CmsAdminPage, CmsAdminPageRevision, CmsAdminWorkspace};
    use coil_auth::DefaultAuthModelPackage;
    use coil_commerce::CommerceModule;
    use coil_config::PlatformConfig;
    use coil_customer_sdk::{
        AuditFacade, AuthFacade, BackendError, BackendErrorKind, CheckoutHooks, CommerceFacade,
        CustomerBackendPlugin, CustomerHookRegistry, CustomerPluginDescriptor, MergePolicy,
        OrderDraft, OrderReviewDecision, RenderModelContribution, RenderModelHooks, RenderTarget,
        RepositoryFacade, RequestContext,
    };
    use coil_observability::TraceRecord;
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
                    model.with_value(
                        "template_name",
                        RenderValue::text(target.template_name.clone()),
                    )
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

    #[test]
    fn admin_diagnostic_traces_reads_duration_from_trace_fields() {
        let app_name = format!(
            "render-diagnostics-{}",
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
                    "local_root = \"/tmp/coil-runtime-tests\"",
                    &format!("local_root = \"{storage_root}\""),
                ),
        )
        .unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        assert!(plan.observability.telemetry.record_trace(
            TraceRecord::new("trace-1", "http.request", "ok", 42).with_field("duration_ms", "17")
        ));

        let traces = admin_diagnostic_traces(Some(&plan)).unwrap();
        let trace = traces.first().cloned().expect("trace row should exist");
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("trace").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="duration" coil:text="${trace.duration_ms}">duration</p>
    <p class="recorded" coil:text="${trace.recorded_at_unix_seconds}">recorded</p>
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
                    TemplateSelector::new(TemplateName::new("trace").unwrap()),
                    RenderModel::new().with_object("trace", trace).unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains(r#"<p class="duration">17</p>"#), "{html}");
        assert!(html.contains(r#"<p class="recorded">42</p>"#), "{html}");
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
            RenderModel::new()
                .with_object(
                    "page",
                    page_model_for_route_name(
                        route_name,
                        &BTreeMap::new(),
                        "Shoppr",
                        route_name.replace('.', "/").as_str(),
                        None,
                    ),
                )
                .unwrap(),
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
            RenderModel::new()
                .with_object(
                    "page",
                    page_model_for_route_name(
                        "memberships.account",
                        &BTreeMap::new(),
                        "Shoppr",
                        "memberships/account",
                        None,
                    ),
                )
                .unwrap(),
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
            RenderModel::new()
                .with_object(
                    "page",
                    page_model_for_route_name(
                        "memberships.account.dashboard",
                        &BTreeMap::new(),
                        "Shoppr",
                        "memberships/account/dashboard",
                        None,
                    ),
                )
                .unwrap(),
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

    fn cms_workspace_fixture() -> CmsAdminWorkspace {
        CmsAdminWorkspace {
            pages: vec![CmsAdminPage {
                id: "page-home".to_string(),
                draft: CmsAdminPageRevision {
                    title: "Editorial Home".to_string(),
                    slug: "home".to_string(),
                    summary: "A draft landing page".to_string(),
                    body_html: "<section><h2>Welcome</h2><p>Body copy</p></section>".to_string(),
                    settings: crate::cms_admin::CmsAdminPageSettings {
                        page_type: "landing_page".to_string(),
                        template: Some("pages/home".to_string()),
                        seo_title: Some("Editorial Home".to_string()),
                        seo_description: Some("A draft landing page".to_string()),
                        options: crate::cms_admin::CmsAdminPageOptions {
                            show_in_navigation: true,
                            allow_indexing: true,
                            localized: false,
                        },
                    },
                    blocks: vec![crate::cms_admin::CmsAdminPageBlock::Instance(
                        crate::cms_admin::CmsAdminBlockInstance {
                            id: "block-home-draft".to_string(),
                            block_type: "rich_text".to_string(),
                            label: Some("Draft body".to_string()),
                            enabled: true,
                            fields: std::collections::BTreeMap::from([(
                                "html".to_string(),
                                "<section><h2>Welcome</h2><p>Body copy</p></section>".to_string(),
                            )]),
                        },
                    )],
                },
                live: Some(CmsAdminPageRevision {
                    title: "Editorial Home".to_string(),
                    slug: "home".to_string(),
                    summary: "A live landing page".to_string(),
                    body_html: "<section><h2>Welcome live</h2><p>Body copy</p></section>"
                        .to_string(),
                    settings: crate::cms_admin::CmsAdminPageSettings {
                        page_type: "landing_page".to_string(),
                        template: Some("pages/home".to_string()),
                        seo_title: Some("Editorial Home".to_string()),
                        seo_description: Some("A live landing page".to_string()),
                        options: crate::cms_admin::CmsAdminPageOptions {
                            show_in_navigation: true,
                            allow_indexing: true,
                            localized: false,
                        },
                    },
                    blocks: vec![
                        crate::cms_admin::CmsAdminPageBlock::Instance(
                            crate::cms_admin::CmsAdminBlockInstance {
                                id: "block-home-live".to_string(),
                                block_type: "rich_text".to_string(),
                                label: Some("Live body".to_string()),
                                enabled: true,
                                fields: std::collections::BTreeMap::from([(
                                    "html".to_string(),
                                    "<section><h2>Welcome live</h2><p>Body copy</p></section>"
                                        .to_string(),
                                )]),
                            },
                        ),
                        crate::cms_admin::CmsAdminPageBlock::SharedReference(
                            crate::cms_admin::CmsAdminSharedBlockReference {
                                id: "block-home-membership-cta".to_string(),
                                shared_block_id: "shared-membership-cta".to_string(),
                                label: Some("Membership CTA".to_string()),
                                enabled: true,
                            },
                        ),
                    ],
                }),
                previous_live: None,
                scheduled_publish_at: None,
                published_once: true,
                updated_at: 1_710_000_000,
            }],
            navigation: Vec::new(),
            redirects: Vec::new(),
            shared_blocks: vec![crate::cms_admin::CmsAdminSharedBlock {
                id: "shared-membership-cta".to_string(),
                label: "Membership CTA".to_string(),
                block_type: "callout".to_string(),
                fields: std::collections::BTreeMap::from([
                    ("heading".to_string(), "Join Shoppr+".to_string()),
                    (
                        "body".to_string(),
                        "Unlock members-only drops and event booking.".to_string(),
                    ),
                    ("cta_href".to_string(), "/en-GB/account".to_string()),
                ]),
                updated_at: 1_710_000_000,
            }],
            global_settings: crate::cms_admin::default_workspace().global_settings,
        }
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
    fn route_specific_model_exposes_discovery_hubs_for_collections_route() {
        let html = render_fixture(
            "commerce.collections",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <article coil:each="hub : ${discovery_hubs}">
      <h2 coil:text="${hub.journey_title}">Journey</h2>
      <p class="count" coil:text="${hub.product_count}">0</p>
    </article>
  </body>
</html>"#,
        );

        assert!(html.contains("Merchandising discovery"), "{html}");
        assert!(html.contains("Membership-led discovery"), "{html}");
        assert!(html.contains("Event-led discovery"), "{html}");
    }

    #[test]
    fn route_specific_model_exposes_structured_page_surface() {
        let html = render_fixture(
            "commerce.product-detail",
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <h1 coil:text="${page.content.title}">title</h1>
    <p class="summary" coil:text="${page.content.summary}">summary</p>
    <p class="template" coil:text="${page.presentation.template}">template</p>
    <p class="fragment" coil:text="${page.presentation.fragment_mode}">false</p>
    <p class="nav" coil:text="${page.settings.show_in_navigation}">true</p>
    <p class="blocks" coil:text="${page.block_count}">0</p>
  </body>
</html>"#,
        );

        assert!(html.contains("Harbor Cap"), "{html}");
        assert!(
            html.contains("Product detail, pricing, and purchase intent"),
            "{html}"
        );
        assert!(html.contains("commerce/product-detail"), "{html}");
        assert!(html.contains(">false<"), "{html}");
        assert!(html.contains(">true<"), "{html}");
        assert!(html.contains(">0<"), "{html}");
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
    fn cms_live_page_model_exposes_structured_content_and_structured_blocks() {
        let model = cms_live_page_model(&cms_workspace_fixture(), "home").unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <h1 coil:text="${cms_page.content.title}">title</h1>
    <p class="summary" coil:text="${cms_page.content.summary}">summary</p>
    <p class="nav" coil:text="${cms_page.settings.show_in_navigation}">true</p>
    <p class="count" coil:text="${cms_page.block_count}">1</p>
    <article coil:each="block : ${cms_page.blocks}">
      <p class="type" coil:text="${block.type_id}">rich_text</p>
      <div class="html" coil:utext="${block.html}">body</div>
      <p
        class="heading"
        coil:if="${block.type_id == 'callout'}"
        coil:text="${block.fields.heading}"
      >
        heading
      </p>
    </article>
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
                    RenderModel::new().with_object("cms_page", model).unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Editorial Home"), "{html}");
        assert!(html.contains("A live landing page"), "{html}");
        assert!(html.contains(">true<"), "{html}");
        assert!(html.contains(">2<"), "{html}");
        assert!(html.contains("rich_text"), "{html}");
        assert!(html.contains("Welcome live"), "{html}");
        assert!(html.contains("callout"), "{html}");
        assert!(html.contains("Join Shoppr+"), "{html}");
    }

    #[test]
    fn cms_live_page_model_resolves_shared_block_references_without_admin_form_fields() {
        let model = cms_live_page_model(&cms_workspace_fixture(), "home").unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <article coil:each="block : ${cms_page.blocks}">
      <p class="kind" coil:text="${block.kind}">kind</p>
      <p class="source" coil:text="${block.source_kind}">source</p>
      <p class="type" coil:text="${block.type_id}">type</p>
      <p class="shared-id" coil:text="${block.shared_block_id}">shared</p>
      <p class="shared-label" coil:text="${block.shared_block_label}">label</p>
    </article>
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
                    RenderModel::new().with_object("cms_page", model).unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(
            html.contains(r#"<p class="kind">shared_reference</p>"#),
            "{html}"
        );
        assert!(html.contains(r#"<p class="source">shared</p>"#), "{html}");
        assert!(html.contains(r#"<p class="type">callout</p>"#), "{html}");
        assert!(
            html.contains(r#"<p class="shared-id">shared-membership-cta</p>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<p class="shared-label">Membership CTA</p>"#),
            "{html}"
        );
    }

    #[test]
    fn cms_live_page_model_filters_disabled_blocks_from_live_output() {
        let mut workspace = cms_workspace_fixture();
        if let Some(CmsAdminPageBlock::SharedReference(reference)) = workspace.pages[0]
            .live
            .as_mut()
            .and_then(|revision| revision.blocks.get_mut(1))
        {
            reference.enabled = false;
        }

        let model = cms_live_page_model(&workspace, "home").unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <p class="count" coil:text="${cms_page.block_count}">1</p>
    <article coil:each="block : ${cms_page.blocks}">
      <p class="type" coil:text="${block.type_id}">type</p>
    </article>
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
                    RenderModel::new().with_object("cms_page", model).unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains(r#"<p class="count">1</p>"#), "{html}");
        assert!(html.contains("rich_text"), "{html}");
        assert!(!html.contains("callout"), "{html}");
    }

    #[test]
    fn cms_admin_selected_page_model_exposes_structured_editor_content() {
        let workspace = cms_workspace_fixture();
        let page = workspace.pages.first().cloned().unwrap();
        let model = cms_admin_selected_page_model_with_form_state(
            Some(page),
            None,
            &workspace.shared_blocks,
        )
        .unwrap();
        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let template = TemplateSourceParser::new()
            .parse_layout(
                namespace.clone(),
                TemplateName::new("page").unwrap(),
                r#"<!doctype html>
<html xmlns:coil="https://coil.rs">
  <body>
    <h1 coil:text="${selected_page.content.title}">title</h1>
    <p class="summary" coil:text="${selected_page.content.summary}">summary</p>
    <p class="layout" coil:text="${selected_page.settings.has_layout_variant}">false</p>
    <p class="count" coil:text="${selected_page.content.block_count}">1</p>
    <section coil:each="block : ${selected_page.blocks}">
      <p class="mode" coil:text="${block.render_mode}">structured_fields</p>
      <div class="html" coil:utext="${block.html}">body</div>
    </section>
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
                        .with_object("selected_page", model)
                        .unwrap(),
                ),
            )
            .unwrap()
            .html;

        assert!(html.contains("Editorial Home"), "{html}");
        assert!(html.contains("A draft landing page"), "{html}");
        assert!(html.contains(">false<"), "{html}");
        assert!(html.contains(">1<"), "{html}");
        assert!(html.contains("structured_fields"), "{html}");
        assert!(html.contains("Welcome"), "{html}");
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
