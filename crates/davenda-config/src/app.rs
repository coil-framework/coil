use std::net::SocketAddr;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub name: String,
    pub environment: Environment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    pub bind: String,
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
    #[serde(default)]
    pub max_body_bytes: Option<usize>,
}

impl ServerConfig {
    pub fn trusts_forwarded_headers(&self, remote_addr: Option<&SocketAddr>) -> bool {
        let Some(remote_addr) = remote_addr else {
            return false;
        };

        self.trusted_proxies.iter().any(|trusted_proxy| {
            trusted_proxy
                .parse::<IpNet>()
                .map(|network| network.contains(&remote_addr.ip()))
                .unwrap_or(false)
        })
    }
}
