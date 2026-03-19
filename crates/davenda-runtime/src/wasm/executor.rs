use super::auth_backend::RuntimeAuthBackend;
use super::host::RuntimeWasmHostServices;
use super::support::{
    runtime_auth_backend_error, runtime_data_backend_error, runtime_executor_error,
    runtime_host_service_error, storage_class_from_grant, trace_id,
};
use super::*;
use davenda_wasm::MetadataGrant;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug)]
pub(super) struct RuntimeHostServiceExecutor {
    plan: RuntimePlan,
    auth_backend: OnceLock<Result<RuntimeAuthBackend, String>>,
    data_backend: OnceLock<Result<RuntimeDataBackend, String>>,
    services: RuntimeWasmHostServices,
}

impl RuntimeHostServiceExecutor {
    pub(super) fn with_services(plan: RuntimePlan, services: RuntimeWasmHostServices) -> Self {
        Self {
            services,
            plan,
            auth_backend: OnceLock::new(),
            data_backend: OnceLock::new(),
        }
    }

    fn execute_auth(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &AuthServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let backend = self.auth_backend()?;
        let execution = backend.execute(request, context, self.plan.tenant_id())?;
        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Auth(execution),
        })
    }

    fn auth_backend(&self) -> Result<&RuntimeAuthBackend, WasmModelError> {
        let result = self.auth_backend.get_or_init(|| {
            RuntimeAuthBackend::new(&self.plan).map_err(|reason| {
                runtime_auth_backend_error(self.plan.tenant_id(), reason).to_string()
            })
        });

        result.as_ref().map_err(|reason: &String| {
            runtime_auth_backend_error(self.plan.tenant_id(), reason.clone())
        })
    }

    fn execute_data(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &DataServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self.data_backend(context)?.execute(request, context)?;

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Data(execution),
        })
    }

    fn data_backend(
        &self,
        context: &InvocationContext,
    ) -> Result<&RuntimeDataBackend, WasmModelError> {
        let result = self.data_backend.get_or_init(|| {
            RuntimeDataBackend::new(&self.plan).map_err(|reason| reason.to_string())
        });

        result
            .as_ref()
            .map_err(|reason| runtime_data_backend_error(context, reason.clone()))
    }

    fn execute_storage(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &StorageServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let (storage_class, bytes) = match request {
            StorageServiceRequest::Read { class } => (*class, 0),
            StorageServiceRequest::Write { class, bytes } => (*class, *bytes),
        };
        let trace_id = trace_id(context);
        let logical_path = format!(
            "wasm/{}/{}/{}",
            context.customer_app.app_id, trace_id, storage_class
        );
        let plan = self
            .plan
            .storage_host()
            .plan_write(
                StoragePlanRequest::new(logical_path)
                    .with_storage_class(storage_class_from_grant(storage_class)),
            )
            .map_err(|error| runtime_executor_error(context, error))?;
        let description = format!(
            "{} via {}",
            plan.logical_path,
            plan.primary_write_target()
                .map(|target| target.locator.as_str())
                .unwrap_or("local")
        );

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Storage(StorageServiceExecution {
                request: request.clone(),
                description,
                total_bytes: bytes,
            }),
        })
    }

    fn execute_render(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &RenderServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let fragment = self.render_fragment(request, context)?;
        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Render(RenderServiceExecution {
                request: request.clone(),
                fragment,
            }),
        })
    }

    fn execute_cache_intent(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &CacheIntentServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let trace_id = trace_id(context);
        let cache_namespace = CacheNamespace::new(format!("wasm:{}", context.customer_app.app_id))
            .map_err(|error| runtime_executor_error(context, error))?;
        let mut scope = if context.principal.id.is_some() {
            CacheScope::private()
        } else {
            CacheScope::public()
        };
        if let Some(locale) = context.customer_app.locale.as_deref() {
            scope = scope
                .with_locale(locale.to_string())
                .map_err(|error| runtime_executor_error(context, error))?;
        }
        scope = scope
            .with_site(context.customer_app.app_id.clone())
            .map_err(|error| runtime_executor_error(context, error))?;
        let freshness =
            FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(30)))
                .expect("constant freshness policy is valid");
        let validators = ResponseValidators {
            etag: Some(
                EntityTag::new(format!(
                    "wasm:{}:{}:cache-intent",
                    context.customer_app.app_id, trace_id
                ))
                .map_err(|error| runtime_executor_error(context, error))?,
            ),
            last_modified_unix_seconds: None,
        };
        let surrogate_tags = InvalidationSet::from_tags([
            InvalidationTag::new(format!("app:{}", context.customer_app.app_id))
                .map_err(|error| runtime_executor_error(context, error))?,
            InvalidationTag::new(format!("trace:{}", trace_id))
                .map_err(|error| runtime_executor_error(context, error))?,
        ]);
        let http_policy =
            HttpCachePolicy::new(scope.clone(), Some(freshness), validators, surrogate_tags)
                .map_err(|error| runtime_executor_error(context, error))?;
        let cache_request =
            CachePlanRequest::new(cache_namespace, format!("wasm:{}", trace_id), http_policy)
                .map_err(|error| runtime_executor_error(context, error))?
                .with_application_policy(
                    ApplicationCachePolicy::new(scope, freshness, InvalidationSet::new())
                        .map_err(|error| runtime_executor_error(context, error))?,
                );
        let plan = self
            .plan
            .cache_planner
            .plan(cache_request)
            .map_err(|error| runtime_executor_error(context, error))?;
        let cache_key = plan
            .application()
            .map(|application| application.key().to_string())
            .unwrap_or_else(|| format!("wasm:{}", trace_id));

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::CacheIntent(CacheIntentExecution {
                request: request.clone(),
                cache_key,
                applied: plan.application().is_some(),
            }),
        })
    }

    fn execute_outbound_http_via_blocking_pool(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        integration: &str,
        response_bytes: u64,
    ) -> Result<HostServiceExecution, WasmModelError> {
        // This method is intentionally just a dispatcher. The outbound HTTP
        // backend offloads the actual network I/O to the blocking pool.
        let execution = self
            .services
            .execute_http_via_blocking_pool(integration, response_bytes, context)
            .map_err(|reason| {
                runtime_host_service_error(context, HostServiceDomain::Network, reason)
            })?;

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Network(execution),
        })
    }

    fn execute_secret(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        secret: &str,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self
            .services
            .read_secret(secret, context)
            .map_err(|reason| {
                runtime_host_service_error(context, HostServiceDomain::Secrets, reason)
            })?;

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Secret(execution),
        })
    }

    fn execute_job(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        queue: &str,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self
            .services
            .enqueue_job(queue, context)
            .map_err(|reason| {
                runtime_host_service_error(context, HostServiceDomain::Jobs, reason)
            })?;

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Job(execution),
        })
    }

    fn execute_metadata(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        kind: MetadataGrant,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self
            .services
            .record_metadata_write(kind, context)
            .map_err(|reason| {
                runtime_host_service_error(context, HostServiceDomain::Metadata, reason)
            })?;

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Metadata(execution),
        })
    }

    fn render_fragment(
        &self,
        request: &RenderServiceRequest,
        context: &InvocationContext,
    ) -> Result<String, WasmModelError> {
        let slot = match request {
            RenderServiceRequest::Fragment { slot } => slot,
        };
        let fragment_name = TemplateName::new(format!("wasm-host-{slot}"))
            .map_err(|error| runtime_executor_error(context, error))?;
        let definition = TemplateDefinition::fragment(
            self.plan.template.customer_app_namespace.clone(),
            fragment_name.clone(),
            vec![Node::Element(
                ElementNode::new(
                    "div",
                    vec![Node::static_text(format!(
                        "host-render:{}:{}",
                        context.customer_app.app_id, slot
                    ))],
                )
                .map_err(|error| runtime_executor_error(context, error))?
                .with_attribute(
                    AttributeNode::static_value("data-slot", slot)
                        .map_err(|error| runtime_executor_error(context, error))?,
                )
                .with_attribute(
                    AttributeNode::static_value("data-app", context.customer_app.app_id.clone())
                        .map_err(|error| runtime_executor_error(context, error))?,
                )
                .with_attribute(
                    AttributeNode::static_value(
                        "data-locale",
                        context
                            .customer_app
                            .locale
                            .clone()
                            .unwrap_or_else(|| self.plan.config.i18n.default_locale.clone()),
                    )
                    .map_err(|error| runtime_executor_error(context, error))?,
                ),
            )],
        );
        let mut registry = self.plan.template.registry.clone();
        registry
            .register(definition)
            .map_err(|error| runtime_executor_error(context, error))?;
        let runtime = TemplateRuntime::new(registry);
        let selector = TemplateSelector::new(fragment_name);
        let model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(context.customer_app.app_id.clone()),
            )
            .map_err(|error| runtime_executor_error(context, error))?
            .with_value("slot", RenderValue::text(slot.clone()))
            .map_err(|error| runtime_executor_error(context, error))?
            .with_value(
                "locale",
                RenderValue::text(
                    context
                        .customer_app
                        .locale
                        .clone()
                        .unwrap_or_else(|| self.plan.config.i18n.default_locale.clone()),
                ),
            )
            .map_err(|error| runtime_executor_error(context, error))?;

        runtime
            .render_fragment(
                &[self.plan.template.customer_app_namespace.clone()],
                FragmentRenderRequest::new(selector, model),
            )
            .map(|output| output.html)
            .map_err(|error| runtime_executor_error(context, error))
    }
}

impl HostServiceExecutor for RuntimeHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError> {
        match &call.request {
            HostServiceRequest::Auth(request) => self.execute_auth(call, context, request),
            HostServiceRequest::Data(request) => self.execute_data(call, context, request),
            HostServiceRequest::Storage(request) => self.execute_storage(call, context, request),
            HostServiceRequest::Render(request) => self.execute_render(call, context, request),
            HostServiceRequest::CacheIntent(request) => {
                self.execute_cache_intent(call, context, request)
            }
            HostServiceRequest::OutboundHttp {
                integration,
                response_bytes,
            } => self.execute_outbound_http_via_blocking_pool(
                call,
                context,
                integration,
                *response_bytes,
            ),
            HostServiceRequest::SecretRead { secret } => self.execute_secret(call, context, secret),
            HostServiceRequest::EnqueueJob { queue } => self.execute_job(call, context, queue),
            HostServiceRequest::MetadataWrite { kind } => {
                self.execute_metadata(call, context, *kind)
            }
        }
    }
}

#[cfg(test)]
mod tests;
