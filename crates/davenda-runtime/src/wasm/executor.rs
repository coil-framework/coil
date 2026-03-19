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
use url::Url;

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

    fn execute_network(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        integration: &str,
        response_bytes: u64,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self
            .services
            .execute_http(integration, response_bytes, context)
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
            } => self.execute_network(call, context, integration, *response_bytes),
            HostServiceRequest::SecretRead { secret } => self.execute_secret(call, context, secret),
            HostServiceRequest::EnqueueJob { queue } => self.execute_job(call, context, queue),
            HostServiceRequest::MetadataWrite { kind } => {
                self.execute_metadata(call, context, *kind)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeBuilder;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_config::PlatformConfig;
    use davenda_wasm::{
        CustomerAppContext, ExtensionId, HandlerId, HostCapabilityGrant, HostGrantSet, HttpMethod,
        InvocationContext, InvocationInput, InvocationPlan, JobExecution, MetadataExecution,
        MetadataGrant, NetworkExecution, PageInvocation, PrincipalRef, ResourceLimits,
        SecretExecution, TraceContext,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;

    const TEST_CONFIG: &str = r#"
[app]
name = "wasm-host-tests"
environment = "development"

[server]
bind = "127.0.0.1:0"
trusted_proxies = []

[http.session]
store = "memory"
idle_timeout_secs = 3600
absolute_timeout_secs = 7200

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = false
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "disabled"
deployment = "distributed"
local_root = "/tmp/davenda-runtime-tests"

[cache]
l1 = "moka"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"
localized_routes = false

[seo]
canonical_host = "example.test"
emit_json_ld = false

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms-pages"]

[wasm]
directory = "/tmp/davenda-wasm-tests"
default_time_limit_ms = 50
allow_network = true

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#;

    #[test]
    fn runtime_host_service_executor_uses_live_backends() {
        let plan = RuntimeBuilder::new(
            PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
            DefaultAuthModelPackage::default(),
        )
        .build()
        .unwrap();
        let (endpoint, server) = spawn_http_server("live-response");

        let mut http_targets = BTreeMap::new();
        http_targets.insert("crm".to_string(), Url::parse(&endpoint).unwrap());
        let mut secrets = BTreeMap::new();
        secrets.insert("api-token".to_string(), "super-secret".to_string());

        let shared_root = shared_state_root("metadata");
        let services = RuntimeWasmHostServices::with_shared_state_root(
            shared_root.clone(),
            plan.clone(),
            http_targets,
            secrets,
        );
        let executor = RuntimeHostServiceExecutor::with_services(plan.clone(), services.clone());
        let session_plan = InvocationPlan {
            extension_id: ExtensionId::new("extensions.live").unwrap(),
            handler_id: HandlerId::new("handler-live").unwrap(),
            point: ExtensionPointKind::Page,
            customer_app_id: "customer-app".to_string(),
            granted_capabilities: HostGrantSet::from_grants([
                HostCapabilityGrant::OutboundHttp {
                    integration: "crm".to_string(),
                },
                HostCapabilityGrant::SecretRead {
                    secret: "api-token".to_string(),
                },
                HostCapabilityGrant::EnqueueJob {
                    queue: "jobs.work".to_string(),
                },
                HostCapabilityGrant::MetadataWrite {
                    kind: MetadataGrant::JsonLd,
                },
            ]),
            limits: ResourceLimits::baseline_for(ExtensionPointKind::Page),
            context: InvocationContext::new(
                CustomerAppContext::new("customer-app").unwrap(),
                PrincipalRef::user("user-1").unwrap(),
                TraceContext::new("trace-1").unwrap(),
                InvocationInput::Page(
                    PageInvocation::new("/host-side-effects", HttpMethod::Get).unwrap(),
                ),
            ),
        };

        let mut session = session_plan.begin_execution_with_executor(Arc::new(executor));

        let network = session
            .execute_host_call(davenda_wasm::HostCall::OutboundHttp {
                integration: "crm".to_string(),
                response_bytes: "live-response".len() as u64,
            })
            .unwrap();
        assert!(matches!(
            network.result,
            HostServiceResult::Network(NetworkExecution {
                integration,
                endpoint: recorded_endpoint,
                status,
                response_bytes,
            }) if integration == "crm"
                && recorded_endpoint == Url::parse(&endpoint).unwrap().to_string()
                && status == 200
                && response_bytes == "live-response".len() as u64
        ));
        assert_eq!(
            session.usage().outbound_response_bytes,
            "live-response".len() as u64
        );

        let secret = session
            .execute_host_call(davenda_wasm::HostCall::SecretRead {
                secret: "api-token".to_string(),
            })
            .unwrap();
        assert!(matches!(
            secret.result,
            HostServiceResult::Secret(SecretExecution {
                secret,
                source,
                value_bytes,
            }) if secret == "api-token"
                && source == "in-memory:api-token"
                && value_bytes == "super-secret".len()
        ));

        let job = session
            .execute_host_call(davenda_wasm::HostCall::EnqueueJob {
                queue: "jobs.work".to_string(),
            })
            .unwrap();
        assert!(matches!(
            job.result,
            HostServiceResult::Job(JobExecution {
                queue,
                job_id,
                enqueued_at_unix_seconds,
            }) if queue == "jobs.work"
                && job_id.starts_with("wasm:")
                && enqueued_at_unix_seconds > 0
        ));

        let jobs = plan.jobs_host("scheduler-a").unwrap();
        assert_eq!(jobs.coordinator().ready_jobs().len(), 1);
        assert_eq!(
            jobs.coordinator().ready_jobs()[0].spec.queue.as_str(),
            "jobs.work"
        );

        let metadata = session
            .execute_host_call(davenda_wasm::HostCall::MetadataWrite {
                kind: MetadataGrant::JsonLd,
            })
            .unwrap();
        assert!(matches!(
            metadata.result,
            HostServiceResult::Metadata(MetadataExecution {
                kind: MetadataGrant::JsonLd,
                recorded: true,
                journal_entries: 1,
            })
        ));

        let metadata = session
            .execute_host_call(davenda_wasm::HostCall::MetadataWrite {
                kind: MetadataGrant::JsonLd,
            })
            .unwrap();
        assert!(matches!(
            metadata.result,
            HostServiceResult::Metadata(MetadataExecution {
                kind: MetadataGrant::JsonLd,
                recorded: true,
                journal_entries: 2,
            })
        ));
        let reopened = RuntimeWasmHostServices::with_shared_state_root(
            shared_root,
            plan.clone(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let snapshot = reopened.metadata_snapshot(10).unwrap();
        assert_eq!(snapshot.entry_count, 2);
        assert_eq!(snapshot.recent_records.len(), 2);
        assert!(snapshot.path.as_ref().unwrap().exists());
        assert_eq!(snapshot.recent_records[0].kind, "json_ld");
        assert_eq!(snapshot.recent_records[0].trace_id, "trace-1");
        assert_eq!(snapshot.recent_records[0].app_id, "customer-app");
        assert_eq!(snapshot.recent_records[1].kind, "json_ld");
        assert_eq!(snapshot.recent_records[1].trace_id, "trace-1");
        assert_eq!(snapshot.recent_records[1].app_id, "customer-app");

        server.join().unwrap();
    }

    #[test]
    fn runtime_host_service_executor_uses_runtime_scoped_secret_bindings() {
        let plan = RuntimeBuilder::new(
            PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
            DefaultAuthModelPackage::default(),
        )
        .build()
        .unwrap();

        let mut secrets = BTreeMap::new();
        secrets.insert("api-token".to_string(), "super-secret".to_string());

        let services = RuntimeWasmHostServices::with_runtime_secrets(plan.clone(), secrets);
        let executor = RuntimeHostServiceExecutor::with_services(plan.clone(), services.clone());
        let session_plan = InvocationPlan {
            extension_id: ExtensionId::new("extensions.live").unwrap(),
            handler_id: HandlerId::new("handler-live").unwrap(),
            point: ExtensionPointKind::Page,
            customer_app_id: "customer-app".to_string(),
            granted_capabilities: HostGrantSet::from_grants([HostCapabilityGrant::SecretRead {
                secret: "api-token".to_string(),
            }]),
            limits: ResourceLimits::baseline_for(ExtensionPointKind::Page),
            context: InvocationContext::new(
                CustomerAppContext::new("customer-app").unwrap(),
                PrincipalRef::user("user-1").unwrap(),
                TraceContext::new("trace-1").unwrap(),
                InvocationInput::Page(
                    PageInvocation::new("/host-side-effects", HttpMethod::Get).unwrap(),
                ),
            ),
        };

        let mut session = session_plan.begin_execution_with_executor(Arc::new(executor));
        let secret = session
            .execute_host_call(davenda_wasm::HostCall::SecretRead {
                secret: "api-token".to_string(),
            })
            .unwrap();

        assert!(matches!(
            secret.result,
            HostServiceResult::Secret(SecretExecution {
                secret,
                source,
                value_bytes,
            }) if secret == "api-token"
                && source == "runtime:wasm-host-tests:api-token"
                && value_bytes == "super-secret".len()
        ));
    }

    #[test]
    fn runtime_host_service_executor_denies_unbound_secrets_without_env_fallback() {
        let plan = RuntimeBuilder::new(
            PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
            DefaultAuthModelPackage::default(),
        )
        .build()
        .unwrap();

        let services = RuntimeWasmHostServices::new(plan.clone());
        let executor = RuntimeHostServiceExecutor::with_services(plan.clone(), services);
        let session_plan = InvocationPlan {
            extension_id: ExtensionId::new("extensions.live").unwrap(),
            handler_id: HandlerId::new("handler-live").unwrap(),
            point: ExtensionPointKind::Page,
            customer_app_id: "customer-app".to_string(),
            granted_capabilities: HostGrantSet::from_grants([HostCapabilityGrant::SecretRead {
                secret: "api-token".to_string(),
            }]),
            limits: ResourceLimits::baseline_for(ExtensionPointKind::Page),
            context: InvocationContext::new(
                CustomerAppContext::new("customer-app").unwrap(),
                PrincipalRef::user("user-1").unwrap(),
                TraceContext::new("trace-1").unwrap(),
                InvocationInput::Page(
                    PageInvocation::new("/host-side-effects", HttpMethod::Get).unwrap(),
                ),
            ),
        };

        let mut session = session_plan.begin_execution_with_executor(Arc::new(executor));
        let error = session
            .execute_host_call(davenda_wasm::HostCall::SecretRead {
                secret: "api-token".to_string(),
            })
            .unwrap_err();

        assert!(format!("{error:?}").contains("was not provided to runtime"));
    }

    fn shared_state_root(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "davenda-wasm-host-{}-{}",
            std::process::id(),
            label
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn spawn_http_server(body: &'static str) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
        });

        (endpoint, handle)
    }
}
