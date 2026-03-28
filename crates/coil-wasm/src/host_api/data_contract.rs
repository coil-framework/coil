use crate::validation::validate_token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleDataRepositoryKind {
    Owned,
    Shared,
}

impl std::fmt::Display for ModuleDataRepositoryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owned => f.write_str("owned"),
            Self::Shared => f.write_str("shared"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDataRepository {
    pub id: String,
    pub kind: ModuleDataRepositoryKind,
}

impl ModuleDataRepository {
    pub fn owned(id: impl Into<String>) -> Result<Self, crate::error::WasmModelError> {
        Ok(Self {
            id: validate_token("data_repository_id", id.into())?,
            kind: ModuleDataRepositoryKind::Owned,
        })
    }

    pub fn shared(id: impl Into<String>) -> Result<Self, crate::error::WasmModelError> {
        Ok(Self {
            id: validate_token("data_repository_id", id.into())?,
            kind: ModuleDataRepositoryKind::Shared,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDataContract {
    pub owner_extension_id: String,
    pub owner_handler_id: String,
    pub repository: ModuleDataRepository,
    pub resource: String,
}

impl ModuleDataContract {
    pub fn new(
        owner_extension_id: impl Into<String>,
        owner_handler_id: impl Into<String>,
        resource: impl Into<String>,
    ) -> Result<Self, crate::error::WasmModelError> {
        let resource = validate_token("data_contract_resource", resource.into())?;
        let repository = ModuleDataRepository::owned(resource.clone())?;
        Ok(Self {
            owner_extension_id: validate_token(
                "data_contract_owner_extension_id",
                owner_extension_id.into(),
            )?,
            owner_handler_id: validate_token(
                "data_contract_owner_handler_id",
                owner_handler_id.into(),
            )?,
            repository,
            resource,
        })
    }

    pub fn with_repository(
        owner_extension_id: impl Into<String>,
        owner_handler_id: impl Into<String>,
        repository: ModuleDataRepository,
    ) -> Result<Self, crate::error::WasmModelError> {
        let resource = repository.id.clone();
        Ok(Self {
            owner_extension_id: validate_token(
                "data_contract_owner_extension_id",
                owner_extension_id.into(),
            )?,
            owner_handler_id: validate_token(
                "data_contract_owner_handler_id",
                owner_handler_id.into(),
            )?,
            repository,
            resource,
        })
    }

    pub fn owned_repository(
        owner_extension_id: impl Into<String>,
        owner_handler_id: impl Into<String>,
        repository: impl Into<String>,
    ) -> Result<Self, crate::error::WasmModelError> {
        Self::with_repository(
            owner_extension_id,
            owner_handler_id,
            ModuleDataRepository::owned(repository)?,
        )
    }

    pub fn repository_id(&self) -> &str {
        &self.repository.id
    }

    pub fn summary(&self, access: &str, sequence: u64) -> String {
        format!(
            "module={} handler={} repository={} repository_kind={} access={} sequence={sequence}",
            self.owner_extension_id,
            self.owner_handler_id,
            self.repository.id,
            self.repository.kind,
            access,
        )
    }
}
