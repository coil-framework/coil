use super::BrowserSecurityError;
use super::support::{sign_payload, verify_payload};
use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsrfProtection {
    pub enabled: bool,
    pub field_name: String,
    pub header_name: String,
}

impl CsrfProtection {
    pub fn from_config(config: &HttpCsrfConfig) -> Self {
        Self {
            enabled: config.enabled,
            field_name: config.field_name.clone(),
            header_name: config.header_name.clone(),
        }
    }

    pub fn issue_token(
        &self,
        secret: &[u8],
        session_id: &str,
        action: &str,
    ) -> Result<String, BrowserSecurityError> {
        if !self.enabled {
            return Err(BrowserSecurityError::CsrfDisabled);
        }

        let mut nonce = [0u8; 16];
        OsRng.fill_bytes(&mut nonce);
        let nonce = URL_SAFE_NO_PAD.encode(nonce);
        let payload = format!("{session_id}:{action}:{nonce}");
        let signature = sign_payload(secret, payload.as_bytes())?;
        Ok(format!("v1.{nonce}.{signature}"))
    }

    pub fn verify_token(
        &self,
        secret: &[u8],
        session_id: &str,
        action: &str,
        token: &str,
    ) -> Result<bool, BrowserSecurityError> {
        if !self.enabled {
            return Ok(true);
        }

        let mut parts = token.split('.');
        let version = parts.next();
        let nonce = parts.next();
        let signature = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidCsrfTokenFormat);
        }

        let nonce = nonce.ok_or(BrowserSecurityError::InvalidCsrfTokenFormat)?;
        let signature = signature.ok_or(BrowserSecurityError::InvalidCsrfTokenFormat)?;
        let payload = format!("{session_id}:{action}:{nonce}");

        Ok(verify_payload(secret, payload.as_bytes(), signature).is_ok())
    }
}
