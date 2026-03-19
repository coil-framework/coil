use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStoreTopology {
    Memory,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieProtection {
    Signed,
    Encrypted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookiePolicy {
    pub name: String,
    pub domain: Option<String>,
    pub path: String,
    pub same_site: SameSitePolicy,
    pub secure: bool,
    pub http_only: bool,
    pub protection: CookieProtection,
}

impl CookiePolicy {
    pub fn from_config(config: &HttpCookieConfig) -> Self {
        Self {
            name: config.name.clone(),
            domain: config.domain.clone(),
            path: config.path.clone(),
            same_site: config.same_site,
            secure: config.secure,
            http_only: config.http_only,
            protection: match config.protection {
                ConfigCookieProtection::Signed => CookieProtection::Signed,
                ConfigCookieProtection::Encrypted => CookieProtection::Encrypted,
            },
        }
    }

    pub fn protect(&self, secret: &[u8], value: &str) -> Result<String, BrowserSecurityError> {
        match self.protection {
            CookieProtection::Signed => CookieSigner::new(self.clone()).sign(secret, value),
            CookieProtection::Encrypted => CookieSealer::new(self.clone()).seal(secret, value),
        }
    }

    pub fn unprotect(&self, secret: &[u8], encoded: &str) -> Result<String, BrowserSecurityError> {
        match self.protection {
            CookieProtection::Signed => CookieSigner::new(self.clone()).verify(secret, encoded),
            CookieProtection::Encrypted => CookieSealer::new(self.clone()).open(secret, encoded),
        }
    }

    pub fn render_set_cookie(&self, value: &str, max_age: Option<Duration>) -> String {
        let mut attributes = vec![format!("{}={value}", self.name)];
        attributes.push(format!("Path={}", self.path));

        if let Some(domain) = &self.domain {
            attributes.push(format!("Domain={domain}"));
        }

        if let Some(max_age) = max_age {
            attributes.push(format!("Max-Age={}", max_age.as_secs()));
        }

        attributes.push(format!(
            "SameSite={}",
            match self.same_site {
                SameSitePolicy::Lax => "Lax",
                SameSitePolicy::Strict => "Strict",
                SameSitePolicy::None => "None",
            }
        ));

        if self.secure {
            attributes.push("Secure".to_string());
        }

        if self.http_only {
            attributes.push("HttpOnly".to_string());
        }

        attributes.join("; ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieSigner {
    pub policy: CookiePolicy,
}

impl CookieSigner {
    pub fn new(policy: CookiePolicy) -> Self {
        Self { policy }
    }

    pub fn sign(&self, secret: &[u8], value: &str) -> Result<String, BrowserSecurityError> {
        ensure_cookie_protection(&self.policy, CookieProtection::Signed)?;
        let payload = URL_SAFE_NO_PAD.encode(value.as_bytes());
        let signature = sign_payload(secret, payload.as_bytes())?;
        Ok(format!("v1.{payload}.{signature}"))
    }

    pub fn verify(&self, secret: &[u8], encoded: &str) -> Result<String, BrowserSecurityError> {
        ensure_cookie_protection(&self.policy, CookieProtection::Signed)?;
        let mut parts = encoded.split('.');
        let version = parts.next();
        let payload = parts.next();
        let signature = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidCookieFormat);
        }

        let payload = payload.ok_or(BrowserSecurityError::InvalidCookieFormat)?;
        let signature = signature.ok_or(BrowserSecurityError::InvalidCookieFormat)?;
        verify_payload(secret, payload.as_bytes(), signature)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| BrowserSecurityError::InvalidCookieFormat)?;

        String::from_utf8(bytes).map_err(|_| BrowserSecurityError::InvalidCookieFormat)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieSealer {
    pub policy: CookiePolicy,
}

impl CookieSealer {
    pub fn new(policy: CookiePolicy) -> Self {
        Self { policy }
    }

    pub fn seal(&self, secret: &[u8], value: &str) -> Result<String, BrowserSecurityError> {
        ensure_cookie_protection(&self.policy, CookieProtection::Encrypted)?;
        if secret.is_empty() {
            return Err(BrowserSecurityError::EmptySecret);
        }

        let cipher = cipher_for_secret(secret);
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), value.as_bytes())
            .map_err(|_| BrowserSecurityError::InvalidEncryptedCookiePayload)?;

        Ok(format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(ciphertext)
        ))
    }

    pub fn open(&self, secret: &[u8], encoded: &str) -> Result<String, BrowserSecurityError> {
        ensure_cookie_protection(&self.policy, CookieProtection::Encrypted)?;
        if secret.is_empty() {
            return Err(BrowserSecurityError::EmptySecret);
        }

        let mut parts = encoded.split('.');
        let version = parts.next();
        let nonce = parts.next();
        let ciphertext = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidEncryptedCookieFormat);
        }

        let nonce = nonce.ok_or(BrowserSecurityError::InvalidEncryptedCookieFormat)?;
        let ciphertext = ciphertext.ok_or(BrowserSecurityError::InvalidEncryptedCookieFormat)?;
        let nonce = URL_SAFE_NO_PAD
            .decode(nonce)
            .map_err(|_| BrowserSecurityError::InvalidEncryptedCookieFormat)?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(ciphertext)
            .map_err(|_| BrowserSecurityError::InvalidEncryptedCookieFormat)?;

        let cipher = cipher_for_secret(secret);
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| BrowserSecurityError::InvalidEncryptedCookiePayload)?;

        String::from_utf8(plaintext)
            .map_err(|_| BrowserSecurityError::InvalidEncryptedCookiePayload)
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSecurityServices {
    pub sessions: SessionSecurityServices,
    pub csrf: CsrfProtection,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrowserSecurityError {
    #[error("browser security operations require a non-empty secret")]
    EmptySecret,
    #[error("cookie policy expects {expected:?} protection but was used as {actual:?}")]
    UnexpectedCookieProtection {
        expected: CookieProtection,
        actual: CookieProtection,
    },
    #[error("cookie value is not in the expected signed format")]
    InvalidCookieFormat,
    #[error("signed cookie failed verification")]
    InvalidCookieSignature,
    #[error("encrypted cookie is not in the expected format")]
    InvalidEncryptedCookieFormat,
    #[error("encrypted cookie failed decryption")]
    InvalidEncryptedCookiePayload,
    #[error("CSRF protection is disabled for this runtime")]
    CsrfDisabled,
    #[error("CSRF token is not in the expected format")]
    InvalidCsrfTokenFormat,
}

fn sign_payload(secret: &[u8], payload: &[u8]) -> Result<String, BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn ensure_cookie_protection(
    policy: &CookiePolicy,
    expected: CookieProtection,
) -> Result<(), BrowserSecurityError> {
    if policy.protection == expected {
        Ok(())
    } else {
        Err(BrowserSecurityError::UnexpectedCookieProtection {
            expected,
            actual: policy.protection,
        })
    }
}

fn cipher_for_secret(secret: &[u8]) -> Aes256Gcm {
    let key = Sha256::digest(secret);
    Aes256Gcm::new_from_slice(&key).expect("sha256 produces a 256-bit key")
}

fn verify_payload(
    secret: &[u8],
    payload: &[u8],
    signature: &str,
) -> Result<(), BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| BrowserSecurityError::InvalidCookieSignature)?;
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    mac.verify_slice(&signature)
        .map_err(|_| BrowserSecurityError::InvalidCookieSignature)
}
