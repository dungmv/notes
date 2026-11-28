use crate::CryptoError;
use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub alg: String,
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            alg: "argon2id".to_string(),
            m_kib: 65_536,
            t: 3,
            p: 1,
        }
    }
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey32([u8; 32]);

impl SecretKey32 {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, CryptoError> {
        let bytes: [u8; 32] = slice.try_into().map_err(|_| CryptoError::InvalidFormat)?;
        Ok(Self(bytes))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SecretKey32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

macro_rules! key_newtype {
    ($name:ident) => {
        #[derive(Clone, Zeroize, ZeroizeOnDrop)]
        pub struct $name(SecretKey32);

        impl $name {
            pub fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(SecretKey32::from_bytes(bytes))
            }

            pub fn from_slice(slice: &[u8]) -> Result<Self, CryptoError> {
                Ok(Self(SecretKey32::from_slice(slice)?))
            }

            pub fn as_bytes(&self) -> &[u8; 32] {
                self.0.as_bytes()
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    };
}

key_newtype!(MasterSecret);
key_newtype!(AuthKey);
key_newtype!(KeyEncryptionKey);
key_newtype!(AccountMasterKey);
key_newtype!(EntityWrapKey);
key_newtype!(DataEncryptionKey);
key_newtype!(BlobIndexKey);
key_newtype!(BlobEncryptionKey);
key_newtype!(RecoveryKek);

pub fn random_salt32() -> [u8; 32] {
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);
    salt
}

pub fn random_key32() -> SecretKey32 {
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(bytes.as_mut());
    SecretKey32::from_bytes(*bytes)
}

pub fn derive_master_secret(
    passphrase: &[u8],
    salt: &[u8],
    params: &KdfParams,
) -> Result<MasterSecret, CryptoError> {
    if params.alg != "argon2id" || salt.len() < 8 {
        return Err(CryptoError::InvalidKdfParams);
    }
    let argon_params = Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|_| CryptoError::InvalidKdfParams)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut out = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase, salt, out.as_mut())
        .map_err(|_| CryptoError::InvalidKdfParams)?;
    Ok(MasterSecret::from_bytes(*out))
}

fn hkdf_expand_32(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> Result<[u8; 32], CryptoError> {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut okm = [0u8; 32];
    hk.expand(info, &mut okm)
        .map_err(|_| CryptoError::InvalidFormat)?;
    Ok(okm)
}

impl MasterSecret {
    pub fn derive_auth_key(&self) -> Result<AuthKey, CryptoError> {
        let okm = Zeroizing::new(hkdf_expand_32(
            self.as_bytes(),
            None,
            b"notes-e2ee-v1:auth-key",
        )?);
        Ok(AuthKey::from_bytes(*okm))
    }

    pub fn derive_kek(&self) -> Result<KeyEncryptionKey, CryptoError> {
        let okm = Zeroizing::new(hkdf_expand_32(
            self.as_bytes(),
            None,
            b"notes-e2ee-v1:kek",
        )?);
        Ok(KeyEncryptionKey::from_bytes(*okm))
    }
}

impl AccountMasterKey {
    pub fn generate() -> Self {
        Self(random_key32())
    }

    pub fn derive_entity_wrap_key(&self) -> Result<EntityWrapKey, CryptoError> {
        let okm = Zeroizing::new(hkdf_expand_32(
            self.as_bytes(),
            None,
            b"notes-e2ee-v1:entity-wrap",
        )?);
        Ok(EntityWrapKey::from_bytes(*okm))
    }

    pub fn derive_blob_index_key(&self) -> Result<BlobIndexKey, CryptoError> {
        let okm = Zeroizing::new(hkdf_expand_32(
            self.as_bytes(),
            None,
            b"notes-e2ee-v1:blob-index",
        )?);
        Ok(BlobIndexKey::from_bytes(*okm))
    }
}

impl RecoveryKek {
    pub fn from_recovery_key(recovery_key: &SecretKey32) -> Result<Self, CryptoError> {
        let okm = Zeroizing::new(hkdf_expand_32(
            recovery_key.as_bytes(),
            None,
            b"notes-e2ee-v1:recovery-kek",
        )?);
        Ok(Self::from_bytes(*okm))
    }
}
