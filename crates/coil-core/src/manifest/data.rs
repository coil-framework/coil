use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRepositoryPrincipalBinding {
    Omit,
    InvocationPrincipal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRepositoryQueryProfile {
    pub page: PageRequest,
    pub publication_visibility: PublicationVisibility,
    pub default_cache_scope: QueryCacheScope,
    pub localized_cache_scope: QueryCacheScope,
    pub principal_binding: DataRepositoryPrincipalBinding,
}

impl DataRepositoryQueryProfile {
    pub fn new(
        page: PageRequest,
        publication_visibility: PublicationVisibility,
        cache_scope: QueryCacheScope,
    ) -> Self {
        Self {
            page,
            publication_visibility,
            default_cache_scope: cache_scope,
            localized_cache_scope: cache_scope,
            principal_binding: DataRepositoryPrincipalBinding::Omit,
        }
    }

    pub fn with_localized_cache_scope(mut self, cache_scope: QueryCacheScope) -> Self {
        self.localized_cache_scope = cache_scope;
        self
    }

    pub fn bind_invocation_principal(mut self) -> Self {
        self.principal_binding = DataRepositoryPrincipalBinding::InvocationPrincipal;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRepositoryContribution {
    pub id: String,
    pub repository: RepositorySpec,
    pub query_profile: DataRepositoryQueryProfile,
}

impl DataRepositoryContribution {
    pub fn new(repository: RepositorySpec, query_profile: DataRepositoryQueryProfile) -> Self {
        Self {
            id: repository.id.clone(),
            repository,
            query_profile,
        }
    }
}
