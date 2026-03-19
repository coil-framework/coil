use super::{BrowserSecurityError, CookiePolicy, CookieProtection};
use crate::*;

pub(super) fn sign_payload(secret: &[u8], payload: &[u8]) -> Result<String, BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

pub(super) fn ensure_cookie_protection(
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

pub(super) fn cipher_for_secret(secret: &[u8]) -> Aes256Gcm {
    let key = Sha256::digest(secret);
    Aes256Gcm::new_from_slice(&key).expect("sha256 produces a 256-bit key")
}

pub(super) fn verify_payload(
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
