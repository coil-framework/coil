use super::support::{
    block_on_auth, describe_tuple_update, runtime_auth_backend_error, subject_for_description,
    subject_for_principal, synthetic_auth_execution, tenant_object,
};
use super::*;
use zanzibar::RebacEngine;

pub(super) struct RuntimeAuthBackend<E = zanzibar::postgres::PostgresRebacEngine> {
    auth: Option<davenda_auth::DavendaAuth<E>>,
    package: davenda_auth::AuthModelPackageSelection,
}

impl<E> std::fmt::Debug for RuntimeAuthBackend<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeAuthBackend")
            .field(
                "tenant_id",
                &self.auth.as_ref().map(|auth| auth.tenant_id()),
            )
            .field("auth_package", &self.package.manifest().name)
            .finish()
    }
}

impl RuntimeAuthBackend<zanzibar::postgres::PostgresRebacEngine> {
    pub(super) fn new(plan: &RuntimePlan) -> Result<Self, String> {
        let package = plan.auth_package.clone();

        match plan.data.connect_lazy_postgres() {
            Ok(client) => {
                let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
                Ok(Self {
                    auth: Some(davenda_auth::DavendaAuth::new(engine, plan.tenant_id())),
                    package,
                })
            }
            Err(error) => {
                #[cfg(test)]
                {
                    let _ = error;
                    Ok(Self {
                        auth: None,
                        package,
                    })
                }
                #[cfg(not(test))]
                {
                    Err(error.to_string())
                }
            }
        }
    }
}

#[cfg(test)]
impl<E> RuntimeAuthBackend<E>
where
    E: RebacEngine + Clone,
{
    pub(crate) fn from_auth(
        auth: davenda_auth::DavendaAuth<E>,
        package: davenda_auth::AuthModelPackageSelection,
    ) -> Self {
        Self {
            auth: Some(auth),
            package,
        }
    }
}

impl<E> RuntimeAuthBackend<E>
where
    E: RebacEngine + Clone,
{
    #[cfg(test)]
    pub(super) fn package(&self) -> &dyn davenda_auth::AuthModelPackage {
        self.package.package()
    }

    pub(super) fn execute(
        &self,
        request: &AuthServiceRequest,
        context: &InvocationContext,
        tenant_id: i64,
    ) -> Result<AuthServiceExecution, WasmModelError> {
        let subject = subject_for_principal(context);
        let tenant = tenant_object(context, tenant_id);
        let principal_id = context.principal.id.clone();
        let Some(auth) = self.auth.as_ref() else {
            return Ok(synthetic_auth_execution(request, context, tenant_id));
        };

        let auth = auth.clone();
        match request {
            AuthServiceRequest::Check => self.execute_check(auth, subject, tenant, principal_id),
            AuthServiceRequest::List => self.execute_list(auth, subject, principal_id),
            AuthServiceRequest::Lookup => self.execute_lookup(auth, tenant, principal_id),
            AuthServiceRequest::TupleWrite => {
                self.execute_tuple_write(auth, subject, tenant, principal_id)
            }
        }
        .map_err(|reason| runtime_auth_backend_error(tenant_id, reason))
    }

    fn execute_check(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        subject: DefaultSubject,
        tenant: Entity,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::SystemConfigRead;
        let package = self.package.package();
        block_on_auth(async move {
            let allowed = auth
                .check_capability(package, &subject, capability, &tenant)
                .await
                .map_err(|error| error.to_string())?;

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::Check,
                allowed,
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::Check {
                    capability: capability.to_string(),
                    object: tenant.to_string(),
                    decision: allowed,
                },
            })
        })
    }

    fn execute_list(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        subject: DefaultSubject,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::CmsPageRead;
        let binding = self
            .package
            .package()
            .binding_for(capability)
            .ok_or_else(|| format!("no capability binding for `{capability}`"))?;
        let namespace = *binding
            .resource_namespaces
            .first()
            .ok_or_else(|| format!("no resource namespace binding for `{capability}`"))?;
        let relation = binding.relation;

        block_on_auth(async move {
            let object_ids = auth
                .list_objects(&subject, relation, namespace)
                .await
                .map_err(|error| error.to_string())?;

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::List,
                allowed: !object_ids.is_empty(),
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::List {
                    capability: capability.to_string(),
                    namespace: namespace.to_string(),
                    object_ids,
                },
            })
        })
    }

    fn execute_lookup(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        tenant: Entity,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::SystemModuleManage;
        let binding = self
            .package
            .package()
            .binding_for(capability)
            .ok_or_else(|| format!("no capability binding for `{capability}`"))?;
        let relation = binding.relation;

        block_on_auth(async move {
            let subject_ids = auth
                .list_subject_ids(&tenant, relation, Namespace::User)
                .await
                .map_err(|error| error.to_string())?;

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::Lookup,
                allowed: !subject_ids.is_empty(),
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::Lookup {
                    capability: capability.to_string(),
                    object: tenant.to_string(),
                    relation: relation.to_string(),
                    subject_namespace: Namespace::User.to_string(),
                    subject_ids,
                },
            })
        })
    }

    fn execute_tuple_write(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        subject: DefaultSubject,
        tenant: Entity,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::SystemConfigWrite;
        let binding = self
            .package
            .package()
            .binding_for(capability)
            .ok_or_else(|| format!("no capability binding for `{capability}`"))?;
        let relation = binding.relation;
        let package = self.package.package();

        block_on_auth(async move {
            let allowed = auth
                .check_capability(package, &subject, capability, &tenant)
                .await
                .map_err(|error| error.to_string())?;
            let updates = vec![DefaultTupleUpdate::Write(DefaultTuple::new(
                tenant.clone(),
                relation,
                subject.clone(),
            ))];
            let written = if allowed {
                auth.write(updates.clone())
                    .await
                    .map_err(|error| error.to_string())?;
                updates.len()
            } else {
                0
            };

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::TupleWrite,
                allowed,
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::TupleWrite {
                    capability: capability.to_string(),
                    object: tenant.to_string(),
                    relation: relation.to_string(),
                    subject: subject_for_description(&subject),
                    updates: updates.iter().map(describe_tuple_update).collect(),
                    written,
                },
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    const TEST_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

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
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
deployment = "distributed"
object_store = "s3"
local_root = "/var/lib/platform"

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
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms-pages", "admin-shell", "memberships", "events", "media-library"]

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

    fn config_with_auth_package(package: &str) -> davenda_config::PlatformConfig {
        davenda_config::PlatformConfig::from_toml_str(
            &TEST_CONFIG.replace("platform-default-auth", package),
        )
        .unwrap()
    }

    #[derive(Debug, Clone)]
    struct TestAuthModelPackage {
        manifest: davenda_auth::AuthModelManifest,
        schema: zanzibar::Schema,
        capability_bindings: HashMap<davenda_auth::Capability, davenda_auth::CapabilityBinding>,
    }

    impl TestAuthModelPackage {
        fn new(name: &str, mode: davenda_auth::PackageMode) -> Self {
            let mut manifest = davenda_auth::default_manifest();
            manifest.name = name.to_string();
            manifest.mode = mode;
            Self {
                manifest,
                schema: davenda_auth::default_schema(),
                capability_bindings: davenda_auth::default_capability_bindings(),
            }
        }
    }

    impl davenda_auth::AuthModelPackage for TestAuthModelPackage {
        fn manifest(&self) -> &davenda_auth::AuthModelManifest {
            &self.manifest
        }

        fn schema(&self) -> &zanzibar::Schema {
            &self.schema
        }

        fn capability_bindings(
            &self,
        ) -> &HashMap<davenda_auth::Capability, davenda_auth::CapabilityBinding> {
            &self.capability_bindings
        }
    }

    #[derive(Debug, Clone, Default)]
    struct MemoryRebacEngine {
        tuples: Arc<Mutex<Vec<zanzibar::Tuple>>>,
    }

    impl MemoryRebacEngine {
        fn with_tuple(self, tuple: zanzibar::Tuple) -> Self {
            self.tuples
                .lock()
                .expect("memory auth engine mutex poisoned")
                .push(tuple);
            self
        }

        fn object(namespace: &str, id: &str) -> zanzibar::Object {
            zanzibar::Object {
                namespace: namespace.to_string(),
                id: id.to_string(),
            }
        }

        fn subject_entity(namespace: &str, id: &str) -> zanzibar::Subject {
            zanzibar::Subject::Entity(Self::object(namespace, id))
        }

        fn tuple(
            object_namespace: &str,
            object_id: &str,
            relation: &str,
            subject_namespace: &str,
            subject_id: &str,
            subject_relation: Option<&str>,
        ) -> zanzibar::Tuple {
            zanzibar::Tuple {
                object: Self::object(object_namespace, object_id),
                relation: relation.to_string(),
                subject: match subject_relation {
                    Some(relation) => zanzibar::Subject::Userset {
                        object: Self::object(subject_namespace, subject_id),
                        relation: relation.to_string(),
                    },
                    None => Self::subject_entity(subject_namespace, subject_id),
                },
            }
        }
    }

    #[async_trait::async_trait]
    impl RebacEngine for MemoryRebacEngine {
        async fn apply_schema(
            &self,
            _tenant_id: i64,
            _schema: zanzibar::Schema,
        ) -> Result<(), zanzibar::RebacError> {
            Ok(())
        }

        async fn write_tuples(
            &self,
            _tenant_id: i64,
            updates: Vec<zanzibar::TupleUpdate>,
        ) -> Result<(), zanzibar::RebacError> {
            let mut tuples = self
                .tuples
                .lock()
                .expect("memory auth engine mutex poisoned");
            for update in updates {
                match update {
                    zanzibar::TupleUpdate::Write(tuple) => {
                        tuples.push(tuple);
                    }
                    zanzibar::TupleUpdate::Delete(tuple) => {
                        tuples.retain(|existing| existing != &tuple);
                    }
                }
            }
            Ok(())
        }

        async fn read_tuples(
            &self,
            _tenant_id: i64,
            object: Option<zanzibar::Object>,
            relation: Option<String>,
            subject: Option<zanzibar::Subject>,
        ) -> Result<Vec<zanzibar::Tuple>, zanzibar::RebacError> {
            let tuples = self
                .tuples
                .lock()
                .expect("memory auth engine mutex poisoned");
            Ok(tuples
                .iter()
                .filter(|tuple| object.as_ref().is_none_or(|expected| &tuple.object == expected))
                .filter(|tuple| relation.as_ref().is_none_or(|expected| &tuple.relation == expected))
                .filter(|tuple| subject.as_ref().is_none_or(|expected| &tuple.subject == expected))
                .cloned()
                .collect())
        }

        async fn check(
            &self,
            _tenant_id: i64,
            subject: &zanzibar::Subject,
            relation: &str,
            object: &zanzibar::Object,
        ) -> Result<bool, zanzibar::RebacError> {
            let tuples = self
                .tuples
                .lock()
                .expect("memory auth engine mutex poisoned");
            Ok(tuples.iter().any(|tuple| {
                &tuple.object == object && tuple.relation == relation && &tuple.subject == subject
            }))
        }

        async fn check_many(
            &self,
            tenant_id: i64,
            requests: Vec<zanzibar::CheckRequest>,
        ) -> Result<Vec<bool>, zanzibar::RebacError> {
            let mut results = Vec::with_capacity(requests.len());
            for request in requests {
                results.push(
                    self.check(tenant_id, &request.subject, &request.relation, &request.object)
                        .await?,
                );
            }
            Ok(results)
        }

        async fn list_objects(
            &self,
            _tenant_id: i64,
            subject: &zanzibar::Subject,
            relation: &str,
            object_namespace: &str,
        ) -> Result<Vec<String>, zanzibar::RebacError> {
            let tuples = self
                .tuples
                .lock()
                .expect("memory auth engine mutex poisoned");
            Ok(tuples
                .iter()
                .filter(|tuple| {
                    tuple.object.namespace == object_namespace
                        && tuple.relation == relation
                        && &tuple.subject == subject
                })
                .map(|tuple| tuple.object.id.clone())
                .collect())
        }

        async fn list_subjects(
            &self,
            _tenant_id: i64,
            object: &zanzibar::Object,
            relation: &str,
            subject_namespace: &str,
        ) -> Result<Vec<String>, zanzibar::RebacError> {
            let tuples = self
                .tuples
                .lock()
                .expect("memory auth engine mutex poisoned");
            Ok(tuples
                .iter()
                .filter(|tuple| {
                    &tuple.object == object
                        && tuple.relation == relation
                        && tuple.subject.namespace() == subject_namespace
                })
                .map(|tuple| tuple.subject.id().to_string())
                .collect())
        }
    }

    fn package_selection(
        name: &str,
        mode: davenda_auth::PackageMode,
    ) -> davenda_auth::AuthModelPackageSelection {
        davenda_auth::AuthModelPackageSelection::new(TestAuthModelPackage::new(name, mode))
    }

    fn invocation_context(principal_id: &str) -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("showcase-events")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user(principal_id).unwrap(),
            TraceContext::new("trace-auth").unwrap(),
            InvocationInput::Api(ApiInvocation::new("/auth", davenda_wasm::HttpMethod::Get).unwrap()),
        )
    }

    #[test]
    fn runtime_auth_backend_new_accepts_replacement_packages_without_hard_failing() {
        let package = TestAuthModelPackage::new(
            "platform-extended-auth",
            davenda_auth::PackageMode::Replace,
        );
        let plan = RuntimeBuilder::new(config_with_auth_package("platform-extended-auth"), package)
            .build()
            .unwrap();
        let backend = RuntimeAuthBackend::new(&plan).unwrap();

        assert_eq!(backend.package().manifest().name, "platform-extended-auth");
        assert_eq!(backend.package().manifest().mode, davenda_auth::PackageMode::Replace);
        assert!(backend.auth.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn runtime_auth_backend_accepts_replacement_packages_and_uses_selected_bindings() {
        let package = package_selection("platform-extended-auth", davenda_auth::PackageMode::Extend);
        let engine = MemoryRebacEngine::default()
            .with_tuple(MemoryRebacEngine::tuple(
                "tenant",
                "101",
                "view",
                "user",
                "alice",
                None,
            ))
            .with_tuple(MemoryRebacEngine::tuple(
                "page",
                "home",
                "view",
                "user",
                "alice",
                None,
            ))
            .with_tuple(MemoryRebacEngine::tuple(
                "tenant",
                "101",
                "manage",
                "user",
                "alice",
                None,
            ));
        let auth = davenda_auth::DavendaAuth::new(engine, 101);
        let backend = RuntimeAuthBackend::from_auth(auth, package);
        assert_eq!(backend.package().manifest().name, "platform-extended-auth");
        assert_eq!(backend.package().manifest().mode, davenda_auth::PackageMode::Extend);

        let context = invocation_context("alice");

        let check = backend
            .execute(&AuthServiceRequest::Check, &context, 101)
            .expect("check execution should succeed");
        assert!(check.allowed);

        let list = backend
            .execute(&AuthServiceRequest::List, &context, 101)
            .expect("list execution should succeed");
        assert!(list.allowed);

        let lookup = backend
            .execute(&AuthServiceRequest::Lookup, &context, 101)
            .expect("lookup execution should succeed");
        assert!(lookup.allowed);

        let tuple_write = backend
            .execute(&AuthServiceRequest::TupleWrite, &context, 101)
            .expect("tuple write execution should succeed");
        assert!(tuple_write.allowed);
        assert!(matches!(
            tuple_write.details,
            AuthServiceDetails::TupleWrite { .. }
        ));
    }
}
