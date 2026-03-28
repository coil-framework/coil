use std::fmt;

use crate::error::WasmModelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ContractVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub const fn is_compatible_with(self, host: Self) -> bool {
        self.major == host.major && self.minor == host.minor && self.patch <= host.patch
    }
}

impl fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionId(String);

impl ExtensionId {
    pub fn new(value: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self(crate::validation::validate_token(
            "extension_id",
            value.into(),
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandlerId(String);

impl HandlerId {
    pub fn new(value: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self(crate::validation::validate_token(
            "handler_id",
            value.into(),
        )?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HandlerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => f.write_str("GET"),
            Self::Head => f.write_str("HEAD"),
            Self::Post => f.write_str("POST"),
            Self::Put => f.write_str("PUT"),
            Self::Patch => f.write_str("PATCH"),
            Self::Delete => f.write_str("DELETE"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtensionPointKind {
    Page,
    Api,
    Job,
    ScheduledJob,
    Webhook,
    AdminWidget,
    RenderHook,
}

impl fmt::Display for ExtensionPointKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page => f.write_str("page"),
            Self::Api => f.write_str("api"),
            Self::Job => f.write_str("job"),
            Self::ScheduledJob => f.write_str("scheduled_job"),
            Self::Webhook => f.write_str("webhook"),
            Self::AdminWidget => f.write_str("admin_widget"),
            Self::RenderHook => f.write_str("render_hook"),
        }
    }
}
