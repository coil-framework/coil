use super::{CookiePolicy, CsrfProtection};
use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStoreTopology {
    Memory,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSecurityServices {
    pub store: SessionStoreTopology,
    pub idle_timeout: Duration,
    pub absolute_timeout: Duration,
    pub session_cookie: CookiePolicy,
    pub flash_cookie: CookiePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSecurityServices {
    pub sessions: SessionSecurityServices,
    pub csrf: CsrfProtection,
}
