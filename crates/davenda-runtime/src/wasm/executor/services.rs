use super::*;
use davenda_wasm::MetadataGrant;
use std::time::Duration;

impl RuntimeHostServiceExecutor {
    pub(super) fn execute_storage(
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
            .services
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

        Ok(self.host_service_execution(
            call,
            HostServiceResult::Storage(StorageServiceExecution {
                request: request.clone(),
                description,
                total_bytes: bytes,
            }),
        ))
    }

    pub(super) fn execute_cache_intent(
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

        Ok(self.host_service_execution(
            call,
            HostServiceResult::CacheIntent(CacheIntentExecution {
                request: request.clone(),
                cache_key,
                applied: plan.application().is_some(),
            }),
        ))
    }

    pub(super) fn dispatch_outbound_http_to_blocking_pool(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        integration: &str,
        response_bytes: u64,
    ) -> Result<HostServiceExecution, WasmModelError> {
        // This method is intentionally just a dispatcher. The outbound HTTP
        // backend submits the actual network I/O to the blocking pool.
        let execution = self
            .services
            .submit_outbound_http_to_blocking_pool(integration, response_bytes, context)
            .map_err(|reason| {
                runtime_host_service_error(context, HostServiceDomain::Network, reason)
            })?;

        Ok(self.host_service_execution(call, HostServiceResult::Network(execution)))
    }

    pub(super) fn execute_secret(
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

        Ok(self.host_service_execution(call, HostServiceResult::Secret(execution)))
    }

    pub(super) fn execute_job(
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

        Ok(self.host_service_execution(call, HostServiceResult::Job(execution)))
    }

    pub(super) fn execute_metadata(
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

        Ok(self.host_service_execution(call, HostServiceResult::Metadata(execution)))
    }
}
