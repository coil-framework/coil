use super::*;
use std::future::Future;
use tokio::runtime::Handle;

pub(super) fn runtime_executor_error(
    context: &InvocationContext,
    error: impl ToString,
) -> WasmModelError {
    WasmModelError::EngineTrap {
        handler_id: trace_id(context).to_string(),
        reason: error.to_string(),
    }
}

pub(super) fn trace_id(context: &InvocationContext) -> &str {
    context.trace.request_id.as_deref().unwrap_or("unknown")
}

pub(super) fn storage_class_from_grant(class: StorageClassGrant) -> StorageClass {
    match class {
        StorageClassGrant::PublicAsset => StorageClass::PublicAsset,
        StorageClassGrant::PublicUpload => StorageClass::PublicUpload,
        StorageClassGrant::PrivateShared => StorageClass::PrivateShared,
        StorageClassGrant::LocalOnlySensitive => StorageClass::LocalOnlySensitive,
    }
}

pub(super) fn http_method_to_wasm(method: HttpMethod) -> WasmHttpMethod {
    match method {
        HttpMethod::Get => WasmHttpMethod::Get,
        HttpMethod::Head => WasmHttpMethod::Head,
        HttpMethod::Post => WasmHttpMethod::Post,
        HttpMethod::Put => WasmHttpMethod::Put,
        HttpMethod::Patch => WasmHttpMethod::Patch,
        HttpMethod::Delete => WasmHttpMethod::Delete,
    }
}

pub(super) fn invocation_surface_path(execution: &RequestExecution) -> String {
    let Some(locale) = execution.route.locale.as_deref() else {
        return execution.path.clone();
    };
    let prefix = format!("/{locale}");
    if execution.path == prefix {
        "/".to_string()
    } else if let Some(rest) = execution.path.strip_prefix(&(prefix.clone() + "/")) {
        format!("/{rest}")
    } else {
        execution.path.clone()
    }
}

pub(super) fn runtime_auth_backend_error(tenant_id: i64, reason: impl ToString) -> WasmModelError {
    WasmModelError::EngineTrap {
        handler_id: format!("auth-tenant-{tenant_id}"),
        reason: reason.to_string(),
    }
}

pub(super) fn runtime_data_backend_error(
    context: &InvocationContext,
    reason: impl ToString,
) -> WasmModelError {
    WasmModelError::HostServiceUnavailable {
        handler_id: trace_id(context).to_string(),
        domain: HostServiceDomain::Data,
        reason: reason.to_string(),
    }
}

pub(super) fn block_on_auth<T>(
    future: impl Future<Output = Result<T, String>> + Send,
) -> Result<T, String>
where
    T: Send,
{
    match Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            runtime.block_on(future)
        }
    }
}

pub(super) fn subject_for_principal(context: &InvocationContext) -> DefaultSubject {
    match context.principal.id.as_deref() {
        Some(principal_id) => match context.principal.kind {
            PrincipalKind::ServiceAccount => {
                DefaultSubject::entity(Entity::service_account(principal_id.to_string()))
            }
            _ => DefaultSubject::entity(Entity::user(principal_id.to_string())),
        },
        None => DefaultSubject::entity(Entity::any_user()),
    }
}

pub(super) fn tenant_object(context: &InvocationContext, tenant_id: i64) -> Entity {
    let object_id = context
        .customer_app
        .tenant_id
        .clone()
        .unwrap_or_else(|| tenant_id.to_string());
    Entity::tenant(object_id)
}

pub(super) fn auth_sequence(context: &InvocationContext) -> u32 {
    context
        .trace
        .trace_id
        .as_bytes()
        .iter()
        .fold(0u32, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(u32::from(*byte))
        })
}

pub(super) fn synthetic_auth_execution(
    request: &AuthServiceRequest,
    context: &InvocationContext,
    tenant_id: i64,
) -> AuthServiceExecution {
    let principal_id = context.principal.id.clone();
    let tenant = tenant_object(context, tenant_id);
    let subject = subject_for_principal(context);
    let decision = true;
    let sequence = auth_sequence(context);

    match request {
        AuthServiceRequest::Check => AuthServiceExecution {
            request: request.clone(),
            allowed: decision,
            checks_seen: 1,
            principal_id,
            details: AuthServiceDetails::Check {
                capability: Capability::SystemConfigRead.to_string(),
                object: tenant.to_string(),
                decision,
            },
        },
        AuthServiceRequest::List => AuthServiceExecution {
            request: request.clone(),
            allowed: true,
            checks_seen: 1,
            principal_id,
            details: AuthServiceDetails::List {
                capability: Capability::CmsPageRead.to_string(),
                namespace: Namespace::Page.to_string(),
                object_ids: vec![format!("synthetic-page-{sequence}")],
            },
        },
        AuthServiceRequest::Lookup => AuthServiceExecution {
            request: request.clone(),
            allowed: true,
            checks_seen: 1,
            principal_id,
            details: AuthServiceDetails::Lookup {
                capability: Capability::SystemModuleManage.to_string(),
                object: tenant.to_string(),
                relation: Relation::Manage.to_string(),
                subject_namespace: Namespace::User.to_string(),
                subject_ids: vec![subject_for_description(&subject)],
            },
        },
        AuthServiceRequest::TupleWrite => AuthServiceExecution {
            request: request.clone(),
            allowed: true,
            checks_seen: 1,
            principal_id,
            details: AuthServiceDetails::TupleWrite {
                capability: Capability::SystemConfigWrite.to_string(),
                object: tenant.to_string(),
                relation: Relation::Manage.to_string(),
                subject: subject_for_description(&subject),
                updates: vec![format!("write {}#manage", tenant)],
                written: 1,
            },
        },
    }
}

pub(super) fn subject_for_description(subject: &DefaultSubject) -> String {
    match subject {
        DefaultSubject::Entity(entity) => entity.to_string(),
        DefaultSubject::Userset { object, relation } => format!("{object}#{relation}"),
    }
}

pub(super) fn describe_tuple_update(update: &DefaultTupleUpdate) -> String {
    match update {
        DefaultTupleUpdate::Write(tuple) => format!(
            "write {}#{}@{}",
            tuple.object,
            tuple.relation,
            subject_for_description(&tuple.subject)
        ),
        DefaultTupleUpdate::Delete(tuple) => format!(
            "delete {}#{}@{}",
            tuple.object,
            tuple.relation,
            subject_for_description(&tuple.subject)
        ),
    }
}
