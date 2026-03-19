use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpConfig {
    pub session: SessionConfig,
    pub session_cookie: CookieConfig,
    pub flash_cookie: CookieConfig,
    pub csrf: CsrfConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    pub store: SessionStore,
    pub idle_timeout_secs: u64,
    pub absolute_timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStore {
    Memory,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CookieConfig {
    pub name: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default = "default_cookie_path")]
    pub path: String,
    pub same_site: SameSitePolicy,
    #[serde(default = "default_true")]
    pub secure: bool,
    #[serde(default = "default_true")]
    pub http_only: bool,
    #[serde(default)]
    pub protection: CookieProtection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SameSitePolicy {
    Lax,
    Strict,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CookieProtection {
    #[default]
    Signed,
    Encrypted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CsrfConfig {
    pub enabled: bool,
    pub field_name: String,
    pub header_name: String,
}

fn default_cookie_path() -> String {
    "/".to_string()
}

fn default_true() -> bool {
    true
}
