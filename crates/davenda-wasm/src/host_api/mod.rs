use crate::grants::{HostCapabilityGrant, MetadataGrant, StorageClassGrant};
use crate::validation::validate_token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDataContract {
    pub owner_extension_id: String,
    pub owner_handler_id: String,
    pub resource: String,
}

impl ModuleDataContract {
    pub fn new(
        owner_extension_id: impl Into<String>,
        owner_handler_id: impl Into<String>,
        resource: impl Into<String>,
    ) -> Result<Self, crate::error::WasmModelError> {
        Ok(Self {
            owner_extension_id: validate_token(
                "data_contract_owner_extension_id",
                owner_extension_id.into(),
            )?,
            owner_handler_id: validate_token(
                "data_contract_owner_handler_id",
                owner_handler_id.into(),
            )?,
            resource: validate_token("data_contract_resource", resource.into())?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostServiceDomain {
    Auth,
    Data,
    Storage,
    Render,
    CacheIntent,
    Network,
    Secrets,
    Jobs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthServiceRequest {
    Check,
    List,
    Lookup,
    TupleWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataServiceRequest {
    Read { contract: ModuleDataContract },
    Write { contract: ModuleDataContract },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageServiceRequest {
    Read {
        class: StorageClassGrant,
    },
    Write {
        class: StorageClassGrant,
        bytes: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderServiceRequest {
    Fragment { slot: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheIntentServiceRequest {
    HintWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostServiceRequest {
    Auth(AuthServiceRequest),
    Data(DataServiceRequest),
    Storage(StorageServiceRequest),
    Render(RenderServiceRequest),
    CacheIntent(CacheIntentServiceRequest),
    OutboundHttp {
        integration: String,
        response_bytes: u64,
    },
    SecretRead {
        secret: String,
    },
    EnqueueJob {
        queue: String,
    },
    MetadataWrite {
        kind: MetadataGrant,
    },
}

impl HostServiceRequest {
    pub fn domain(&self) -> HostServiceDomain {
        match self {
            Self::Auth(_) => HostServiceDomain::Auth,
            Self::Data(_) => HostServiceDomain::Data,
            Self::Storage(_) => HostServiceDomain::Storage,
            Self::Render(_) => HostServiceDomain::Render,
            Self::CacheIntent(_) => HostServiceDomain::CacheIntent,
            Self::OutboundHttp { .. } => HostServiceDomain::Network,
            Self::SecretRead { .. } => HostServiceDomain::Secrets,
            Self::EnqueueJob { .. } => HostServiceDomain::Jobs,
            Self::MetadataWrite { .. } => HostServiceDomain::Render,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostServiceCall {
    pub grant: HostCapabilityGrant,
    pub request: HostServiceRequest,
}

impl HostServiceCall {
    pub fn new(grant: HostCapabilityGrant, request: HostServiceRequest) -> Self {
        Self { grant, request }
    }

    pub fn domain(&self) -> HostServiceDomain {
        self.request.domain()
    }
}
