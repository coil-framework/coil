use std::collections::BTreeMap;

use crate::grants::StorageClassGrant;
use crate::host_api::StorageServiceRequest;
use crate::invocation::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageServiceExecution {
    pub request: StorageServiceRequest,
    pub description: String,
    pub total_bytes: u64,
}

pub(super) fn storage_total_for_request(
    request: &StorageServiceRequest,
    storage_bytes_by_class: &mut BTreeMap<StorageClassGrant, u64>,
    context: &InvocationContext,
) -> u64 {
    let (class, bytes) = match request {
        StorageServiceRequest::Read { class } => (*class, 0),
        StorageServiceRequest::Write { class, bytes } => (*class, *bytes),
    };
    let total = storage_bytes_by_class
        .entry(class)
        .and_modify(|current| *current = current.saturating_add(bytes))
        .or_insert(bytes);
    let _ = context;
    *total
}

pub(super) fn storage_description_for_request(
    request: &StorageServiceRequest,
    context: &InvocationContext,
    total_bytes: u64,
) -> String {
    let app_id = &context.customer_app.app_id;
    match request {
        StorageServiceRequest::Read { class } => {
            format!("read storage class {class} for {app_id}")
        }
        StorageServiceRequest::Write { class, bytes } => format!(
            "write {bytes} bytes to storage class {class} for {app_id} (total {total_bytes} bytes)"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grants::StorageClassGrant;
    use crate::host_api::StorageServiceRequest;
    use crate::invocation::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef,
        TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("storage-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-storage").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/storage", crate::ids::HttpMethod::Get).unwrap(),
            ),
        )
    }

    #[test]
    fn storage_totals_accumulate_by_class() {
        let context = context();
        let mut totals = BTreeMap::new();
        let request = StorageServiceRequest::Write {
            class: StorageClassGrant::PublicUpload,
            bytes: 12,
        };
        let total = storage_total_for_request(&request, &mut totals, &context);
        assert_eq!(total, 12);
        let total = storage_total_for_request(&request, &mut totals, &context);
        assert_eq!(total, 24);
    }
}
