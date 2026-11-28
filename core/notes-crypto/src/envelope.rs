use crate::CryptoError;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};
use uuid::Uuid;

pub const CRYPTO_VERSION_V1: u8 = 0x01;
pub const CIPHER_XCHACHA20_POLY1305: u8 = 0x01;
pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;
pub const ENVELOPE_HEADER_LEN: usize = 1 + 1 + 4 + NONCE_LEN;
pub const ENVELOPE_OVERHEAD: usize = ENVELOPE_HEADER_LEN + TAG_LEN;

#[derive(Clone, Copy, Debug)]
pub struct AadContext<'a> {
    pub context: &'a str,
    pub key_version: u32,
    pub user_id: Uuid,
    pub entity_type: &'a str,
    pub entity_id: Uuid,
    pub client_op_id: Uuid,
}

pub fn build_aad(crypto_ver: u8, cipher_id: u8, ctx: &AadContext<'_>) -> Vec<u8> {
    let context_bytes = ctx.context.as_bytes();
    let entity_type_bytes = ctx.entity_type.as_bytes();
    let mut aad = Vec::with_capacity(
        4 + context_bytes.len() + 1 + 1 + 4 + 16 + 4 + entity_type_bytes.len() + 16 + 16,
    );
    aad.extend_from_slice(&(context_bytes.len() as u32).to_be_bytes());
    aad.extend_from_slice(context_bytes);
    aad.push(crypto_ver);
    aad.push(cipher_id);
    aad.extend_from_slice(&ctx.key_version.to_be_bytes());
    aad.extend_from_slice(ctx.user_id.as_bytes());
    aad.extend_from_slice(&(entity_type_bytes.len() as u32).to_be_bytes());
    aad.extend_from_slice(entity_type_bytes);
    aad.extend_from_slice(ctx.entity_id.as_bytes());
    aad.extend_from_slice(ctx.client_op_id.as_bytes());
    aad
}

pub fn pad_plaintext(plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let raw_len = 4usize
        .checked_add(plaintext.len())
        .ok_or(CryptoError::InvalidFormat)?;
    let target_len = if raw_len <= 256 {
        256
    } else if raw_len <= 1024 {
        raw_len.div_ceil(256) * 256
    } else {
        raw_len.div_ceil(1024) * 1024
    };
    let mut out = Vec::with_capacity(target_len);
    out.extend_from_slice(&(plaintext.len() as u32).to_be_bytes());
    out.extend_from_slice(plaintext);
    out.resize(target_len, 0);
    Ok(out)
}

pub fn unpad_plaintext(padded: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if padded.len() < 4 {
        return Err(CryptoError::InvalidFormat);
    }
    let len = u32::from_be_bytes(padded[0..4].try_into().unwrap()) as usize;
    let end = 4usize.checked_add(len).ok_or(CryptoError::InvalidFormat)?;
    if end > padded.len() {
        return Err(CryptoError::InvalidFormat);
    }
    Ok(padded[4..end].to_vec())
}

pub fn encrypt_envelope(
    key: &[u8; 32],
    plaintext: &[u8],
    aad_ctx: &AadContext<'_>,
) -> Result<Vec<u8>, CryptoError> {
    let padded = pad_plaintext(plaintext)?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let aad = build_aad(CRYPTO_VERSION_V1, CIPHER_XCHACHA20_POLY1305, aad_ctx);
    let cipher = XChaCha20Poly1305::new(key.into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: &padded,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    let mut out = Vec::with_capacity(ENVELOPE_HEADER_LEN + ciphertext.len());
    out.push(CRYPTO_VERSION_V1);
    out.push(CIPHER_XCHACHA20_POLY1305);
    out.extend_from_slice(&aad_ctx.key_version.to_be_bytes());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_envelope(
    key: &[u8; 32],
    envelope: &[u8],
    aad_ctx: &AadContext<'_>,
) -> Result<Vec<u8>, CryptoError> {
    if envelope.len() < ENVELOPE_OVERHEAD {
        return Err(CryptoError::InvalidFormat);
    }
    let crypto_ver = envelope[0];
    let cipher_id = envelope[1];
    if crypto_ver != CRYPTO_VERSION_V1 || cipher_id != CIPHER_XCHACHA20_POLY1305 {
        return Err(CryptoError::UnsupportedVersion);
    }
    let key_version = u32::from_be_bytes(envelope[2..6].try_into().unwrap());
    if key_version != aad_ctx.key_version {
        return Err(CryptoError::AuthenticationFailed);
    }
    let nonce = XNonce::from_slice(&envelope[6..ENVELOPE_HEADER_LEN]);
    let ciphertext = &envelope[ENVELOPE_HEADER_LEN..];

    let aad = build_aad(crypto_ver, cipher_id, aad_ctx);
    let cipher = XChaCha20Poly1305::new(key.into());
    let padded = cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)?;

    unpad_plaintext(&padded)
}
