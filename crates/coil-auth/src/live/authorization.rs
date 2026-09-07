use super::LiveAuthError;
use crate::{AuthModelPackageSelection, Capability, CoilAuth, DefaultSubject, Entity};
use coil_config::PlatformConfig;
use coil_data::DataRuntime;
use std::fmt;
use std::sync::OnceLock;

/// Lazy PostgreSQL-backed capability service shared by every HTTP shell.
///
/// Request adapters remain responsible for deriving a trusted subject and
/// resource. This service only evaluates that explicit authorization question.
pub struct LiveAuthorizationHost {
    data: DataRuntime,
    tenant_id: i64,
    auth_package: AuthModelPackageSelection,
    checker: OnceLock<Result<PostgresCapabilityChecker, String>>,
}

impl LiveAuthorizationHost {
    pub fn from_config(
        config: &PlatformConfig,
        auth_package: AuthModelPackageSelection,
    ) -> Result<Self, LiveAuthError> {
        let data = DataRuntime::from_config(&config.database).map_err(|error| {
            LiveAuthError::BackendInitialization {
                reason: error.to_string(),
            }
        })?;
        Ok(Self::new(data, config.auth.tenant_id, auth_package))
    }

    pub fn new(data: DataRuntime, tenant_id: i64, auth_package: AuthModelPackageSelection) -> Self {
        Self {
            data,
            tenant_id,
            auth_package,
            checker: OnceLock::new(),
        }
    }

    pub async fn check_capability(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<bool, LiveAuthError> {
        self.checker()?.check(subject, capability, object).await
    }

    fn checker(&self) -> Result<&PostgresCapabilityChecker, LiveAuthError> {
        match self.checker.get_or_init(|| self.build_checker()) {
            Ok(checker) => Ok(checker),
            Err(reason) => Err(LiveAuthError::BackendInitialization {
                reason: reason.clone(),
            }),
        }
    }

    fn build_checker(&self) -> Result<PostgresCapabilityChecker, String> {
        let client = self
            .data
            .clone()
            .connect_lazy_postgres()
            .map_err(|error| error.to_string())?;
        let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
        Ok(PostgresCapabilityChecker {
            auth: CoilAuth::new(engine, self.tenant_id),
            package: self.auth_package.clone(),
        })
    }
}

impl fmt::Debug for LiveAuthorizationHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveAuthorizationHost")
            .field("tenant_id", &self.tenant_id)
            .field("auth_package", &self.auth_package.manifest().name)
            .finish_non_exhaustive()
    }
}

struct PostgresCapabilityChecker {
    auth: CoilAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: AuthModelPackageSelection,
}

impl PostgresCapabilityChecker {
    async fn check(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<bool, LiveAuthError> {
        self.auth
            .check_capability(self.package.package(), subject, capability, object)
            .await
            .map_err(|error| LiveAuthError::Authorization {
                reason: error.to_string(),
            })
    }
}
