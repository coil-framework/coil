use std::{collections::BTreeMap, fmt};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{CertificateRecord, TlsModelError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateChainPem(String);

impl CertificateChainPem {
    pub fn new(value: impl Into<String>) -> Result<Self, TlsModelError> {
        let value = value.into();
        validate_pem_block("certificate_chain_pem", &value, "CERTIFICATE")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CertificateChainPem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateKeyPem(String);

impl PrivateKeyPem {
    pub fn new(value: impl Into<String>) -> Result<Self, TlsModelError> {
        let value = value.into();
        validate_pem_block("private_key_pem", &value, "PRIVATE KEY")?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PrivateKeyPem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateMaterial {
    pub certificate_chain_pem: CertificateChainPem,
    pub private_key_pem: PrivateKeyPem,
}

impl CertificateMaterial {
    pub fn new(
        certificate_chain_pem: impl Into<String>,
        private_key_pem: impl Into<String>,
    ) -> Result<Self, TlsModelError> {
        Ok(Self {
            certificate_chain_pem: CertificateChainPem::new(certificate_chain_pem)?,
            private_key_pem: PrivateKeyPem::new(private_key_pem)?,
        })
    }

    pub fn certificate_chain_pem(&self) -> &CertificateChainPem {
        &self.certificate_chain_pem
    }

    pub fn private_key_pem(&self) -> &PrivateKeyPem {
        &self.private_key_pem
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptedCertificateMaterial {
    pub key_id: String,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsMaterialProtector {
    active_key_id: String,
    active_key: [u8; 32],
    decryption_keys: BTreeMap<String, [u8; 32]>,
}

impl TlsMaterialProtector {
    pub fn from_seed(seed: impl AsRef<[u8]>) -> Result<Self, TlsModelError> {
        Self::from_seed_ring(seed, std::iter::empty::<&[u8]>())
    }

    pub fn from_seed_ring<I, S>(active_seed: S, previous_seeds: I) -> Result<Self, TlsModelError>
    where
        I: IntoIterator,
        I::Item: AsRef<[u8]>,
        S: AsRef<[u8]>,
    {
        let (active_key_id, active_key) = derive_key("tls_material_seed", active_seed.as_ref())?;
        let mut decryption_keys = BTreeMap::new();
        decryption_keys.insert(active_key_id.clone(), active_key);

        for seed in previous_seeds {
            let (key_id, key) = derive_key("tls_material_previous_seed", seed.as_ref())?;
            decryption_keys.entry(key_id).or_insert(key);
        }

        Ok(Self {
            active_key_id,
            active_key,
            decryption_keys,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.active_key_id
    }

    pub fn encrypt(
        &self,
        material: &CertificateMaterial,
    ) -> Result<EncryptedCertificateMaterial, TlsModelError> {
        let cipher = Aes256Gcm::new_from_slice(&self.active_key).map_err(|error| {
            TlsModelError::CertificateMaterialEncryptionFailed {
                reason: error.to_string(),
            }
        })?;

        let mut nonce = [0_u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce);

        let payload = serde_json::to_vec(material).map_err(|error| {
            TlsModelError::CertificateMaterialEncryptionFailed {
                reason: error.to_string(),
            }
        })?;

        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), payload.as_slice())
            .map_err(|error| TlsModelError::CertificateMaterialEncryptionFailed {
                reason: error.to_string(),
            })?;

        Ok(EncryptedCertificateMaterial {
            key_id: self.active_key_id.clone(),
            nonce,
            ciphertext,
        })
    }

    pub fn decrypt(
        &self,
        encrypted: &EncryptedCertificateMaterial,
    ) -> Result<CertificateMaterial, TlsModelError> {
        let key = self.decryption_keys.get(&encrypted.key_id).ok_or_else(|| {
            TlsModelError::UnsupportedEncryptedMaterialKey {
                key_id: encrypted.key_id.clone(),
            }
        })?;

        let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| {
            TlsModelError::CertificateMaterialDecryptionFailed {
                reason: error.to_string(),
            }
        })?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&encrypted.nonce),
                encrypted.ciphertext.as_ref(),
            )
            .map_err(|error| TlsModelError::CertificateMaterialDecryptionFailed {
                reason: error.to_string(),
            })?;

        serde_json::from_slice(&plaintext).map_err(|error| {
            TlsModelError::CertificateMaterialDecryptionFailed {
                reason: error.to_string(),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCertificateBundle {
    pub record: CertificateRecord,
    pub material: CertificateMaterial,
}

impl ManualCertificateBundle {
    pub fn new(record: CertificateRecord, material: CertificateMaterial) -> Self {
        Self { record, material }
    }

    pub fn into_encrypted_record(
        mut self,
        protector: &TlsMaterialProtector,
    ) -> Result<CertificateRecord, TlsModelError> {
        if self.record.material.is_some() {
            return Err(TlsModelError::CertificateMaterialAlreadyAttached {
                certificate_id: self.record.id.to_string(),
            });
        }

        let encrypted = protector.encrypt(&self.material)?;
        self.record.material = Some(encrypted);
        Ok(self.record)
    }
}

fn validate_pem_block(
    field: &'static str,
    value: &str,
    marker: &'static str,
) -> Result<(), TlsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(TlsModelError::EmptyField { field });
    }

    if trimmed.chars().any(|ch| ch == '\0') {
        return Err(TlsModelError::InvalidCertificateMaterial {
            field,
            reason: "contains a null byte".to_string(),
        });
    }

    if !trimmed.contains("-----BEGIN") || !trimmed.contains("-----END") {
        return Err(TlsModelError::InvalidCertificateMaterial {
            field,
            reason: "must be PEM encoded".to_string(),
        });
    }

    if !trimmed.contains(marker) {
        return Err(TlsModelError::InvalidCertificateMaterial {
            field,
            reason: format!("must contain `{marker}`"),
        });
    }

    Ok(())
}

fn derive_key(field: &'static str, seed: &[u8]) -> Result<(String, [u8; 32]), TlsModelError> {
    if seed.is_empty() {
        return Err(TlsModelError::EmptyField { field });
    }

    let digest = Sha256::digest(seed);
    let mut key = [0_u8; 32];
    key.copy_from_slice(&digest);
    Ok((format!("{:x}", digest), key))
}
