//! Locked Notes mật mã cục bộ: khóa ghi chú bằng mật khẩu riêng (NLK) độc lập
//! với Account Master Key của đồng bộ. Theo [spec lưu trữ](../../docs/specs/02-data-model-and-storage.md)
//! §3.4 và [spec E2EE](../../docs/specs/07-e2ee-cryptography.md) §7.1:
//! - NLK sinh ngẫu nhiên 32 byte, được wrap bằng KEK dẫn xuất từ mật khẩu khóa
//!   (Argon2id + salt riêng cho từng ghi chú).
//! - `crdt_state`, manifest attachment và các blob ảnh của ghi chú khóa đều ở
//!   dạng ciphertext; không dùng chung khóa với hệ thống đồng bộ.
//! - Mọi envelope dùng AAD gắn bó (context, entity id) để chống tráo dữ liệu
//!   giữa các ghi chú hoặc giữa các loại payload.

use crate::keys::{
    ImageBlobKey, KdfParams, NoteLockKey, SecretKey32, derive_master_secret, random_key32,
    random_salt32,
};
use crate::CryptoError;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

/// Phiên bản envelope cục bộ của Locked Notes.
pub const LOCK_ENVELOPE_VERSION: u8 = 0x01;
/// Phiên bản cấu trúc `NoteLockMetadata`.
pub const LOCK_METADATA_VERSION: u8 = 1;
/// Phiên bản cấu trúc `NoteLockManifest`.
pub const LOCK_MANIFEST_VERSION: u8 = 1;

const HKDF_INFO_LOCK_KEK: &[u8] = b"notes-lock-v1:kek";
/// AAD context cho envelope wrap của NLK trong `lock_metadata`.
pub const LOCK_CONTEXT_NOTE_KEY: &str = "notes-lock-v1:note-key";
/// AAD context cho `crdt_state` đã mã hóa của ghi chú khóa.
pub const LOCK_CONTEXT_STATE: &str = "notes-lock-v1:state";
/// AAD context cho manifest attachment đã mã hóa.
pub const LOCK_CONTEXT_MANIFEST: &str = "notes-lock-v1:manifest";
/// AAD context cho envelope wrap của khóa blob ảnh (bằng NLK).
pub const LOCK_CONTEXT_BLOB_KEY: &str = "notes-lock-v1:blob-key";
/// AAD context cho nội dung blob ảnh đã mã hóa.
pub const LOCK_CONTEXT_BLOB: &str = "notes-lock-v1:blob";

const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;
const HEADER_LEN: usize = 1 + NONCE_LEN;

/// Metadata khóa của một ghi chú khóa, lưu dạng JSON trong `notes.lock_metadata`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteLockMetadata {
    pub version: u8,
    pub kdf: KdfParams,
    /// Salt Argon2id riêng của ghi chú, mã hóa hex.
    pub kdf_salt_hex: String,
    /// NLK đã wrap bằng lock KEK, envelope dạng hex.
    pub wrapped_nlk_hex: String,
    pub hint: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Một attachment của ghi chú khóa, lưu trong manifest đã mã hóa.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockedAttachment {
    pub id: String,
    pub kind: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    /// SHA-256 của bản rõ, dùng để khôi phục đường dẫn blob khi gỡ bảo vệ.
    pub content_hash: String,
    /// Định danh blob đã mã hóa (64 ký tự hex), đường dẫn blob store.
    pub blob_id: String,
    /// Khóa blob ảnh đã wrap bằng NLK, envelope dạng hex.
    pub wrapped_key_hex: String,
    /// Mốc tạo attachment gốc, khôi phục nguyên trạng khi gỡ bảo vệ.
    pub created_at: i64,
}

/// Manifest attachment của ghi chú khóa; được mã hóa bằng NLK trước khi lưu.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteLockManifest {
    pub version: u8,
    pub attachments: Vec<LockedAttachment>,
}

fn lock_aad(context: &str, entity_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(8 + context.len() + entity_id.len());
    aad.extend_from_slice(&(context.len() as u32).to_be_bytes());
    aad.extend_from_slice(context.as_bytes());
    aad.extend_from_slice(&(entity_id.len() as u32).to_be_bytes());
    aad.extend_from_slice(entity_id.as_bytes());
    aad
}

/// Mã hóa một payload nhỏ của Locked Notes (trạng thái, manifest, khóa wrap).
pub fn encrypt_lock_payload(
    key: &[u8; 32],
    entity_id: &str,
    context: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let cipher = XChaCha20Poly1305::new(key.into());
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext,
                aad: &lock_aad(context, entity_id),
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)?;
    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.push(LOCK_ENVELOPE_VERSION);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Giải mã payload của Locked Notes; sai khóa, sai AAD hoặc dữ liệu hỏng đều
/// trả về `AuthenticationFailed`.
pub fn decrypt_lock_payload(
    key: &[u8; 32],
    entity_id: &str,
    context: &str,
    envelope: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if envelope.len() < HEADER_LEN + TAG_LEN || envelope[0] != LOCK_ENVELOPE_VERSION {
        return Err(CryptoError::InvalidFormat);
    }
    let nonce = XNonce::from_slice(&envelope[1..HEADER_LEN]);
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &envelope[HEADER_LEN..],
                aad: &lock_aad(context, entity_id),
            },
        )
        .map_err(|_| CryptoError::AuthenticationFailed)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("high nibble"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("low nibble"));
    }
    out
}

fn hex_decode(input: &str) -> Result<Vec<u8>, CryptoError> {
    if input.len() % 2 != 0 {
        return Err(CryptoError::InvalidFormat);
    }
    (0..input.len() / 2)
        .map(|index| {
            u8::from_str_radix(&input[index * 2..index * 2 + 2], 16)
                .map_err(|_| CryptoError::InvalidFormat)
        })
        .collect()
}

fn lock_kek(passphrase: &[u8], salt: &[u8], params: &KdfParams) -> Result<SecretKey32, CryptoError> {
    let master = derive_master_secret(passphrase, salt, params)?;
    let hk = Hkdf::<Sha256>::new(None, master.as_bytes());
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(HKDF_INFO_LOCK_KEK, okm.as_mut())
        .map_err(|_| CryptoError::InvalidFormat)?;
    Ok(SecretKey32::from_bytes(*okm))
}

/// Sinh NLK ngẫu nhiên cho một ghi chú khóa.
pub fn generate_note_lock_key() -> Result<NoteLockKey, CryptoError> {
    NoteLockKey::from_slice(random_key32().as_bytes())
}

/// Wrap NLK bằng mật khẩu khóa và trả về metadata cần lưu.
#[allow(clippy::too_many_arguments)]
pub fn wrap_note_lock_key(
    note_id: &str,
    passphrase: &[u8],
    nlk: &NoteLockKey,
    kdf: &KdfParams,
    hint: Option<String>,
    created_at: i64,
    updated_at: i64,
) -> Result<NoteLockMetadata, CryptoError> {
    let salt = random_salt32();
    let kek = lock_kek(passphrase, &salt, kdf)?;
    let wrapped = encrypt_lock_payload(
        kek.as_bytes(),
        note_id,
        LOCK_CONTEXT_NOTE_KEY,
        nlk.as_bytes(),
    )?;
    Ok(NoteLockMetadata {
        version: LOCK_METADATA_VERSION,
        kdf: kdf.clone(),
        kdf_salt_hex: hex_encode(&salt),
        wrapped_nlk_hex: hex_encode(&wrapped),
        hint,
        created_at,
        updated_at,
    })
}

/// unwrap NLK từ metadata bằng mật khẩu khóa; sai mật khẩu trả về
/// `AuthenticationFailed`.
pub fn unwrap_note_lock_key(
    note_id: &str,
    passphrase: &[u8],
    metadata: &NoteLockMetadata,
) -> Result<NoteLockKey, CryptoError> {
    if metadata.version != LOCK_METADATA_VERSION {
        return Err(CryptoError::UnsupportedVersion);
    }
    if metadata.kdf_salt_hex.len() != 64 {
        return Err(CryptoError::InvalidFormat);
    }
    let salt = hex_decode(&metadata.kdf_salt_hex)?;
    let kek = lock_kek(passphrase, &salt, &metadata.kdf)?;
    let wrapped = hex_decode(&metadata.wrapped_nlk_hex)?;
    let bytes = decrypt_lock_payload(
        kek.as_bytes(),
        note_id,
        LOCK_CONTEXT_NOTE_KEY,
        &wrapped,
    )?;
    NoteLockKey::from_slice(&bytes)
}

/// Sinh khóa ngẫu nhiên cho một blob ảnh của ghi chú khóa.
pub fn generate_image_blob_key() -> Result<ImageBlobKey, CryptoError> {
    ImageBlobKey::from_slice(random_key32().as_bytes())
}

/// Wrap khóa blob ảnh bằng NLK để lưu trong manifest.
pub fn wrap_image_blob_key(
    note_id: &str,
    nlk: &NoteLockKey,
    key: &ImageBlobKey,
) -> Result<String, CryptoError> {
    let wrapped = encrypt_lock_payload(
        nlk.as_bytes(),
        note_id,
        LOCK_CONTEXT_BLOB_KEY,
        key.as_bytes(),
    )?;
    Ok(hex_encode(&wrapped))
}

/// unwrap khóa blob ảnh từ manifest bằng NLK.
pub fn unwrap_image_blob_key(
    note_id: &str,
    nlk: &NoteLockKey,
    wrapped_key_hex: &str,
) -> Result<ImageBlobKey, CryptoError> {
    let wrapped = hex_decode(wrapped_key_hex)?;
    let bytes = decrypt_lock_payload(
        nlk.as_bytes(),
        note_id,
        LOCK_CONTEXT_BLOB_KEY,
        &wrapped,
    )?;
    ImageBlobKey::from_slice(&bytes)
}

/// Sinh định danh blob ngẫu nhiên (32 byte, 64 ký tự hex) cho blob đã mã hóa.
pub fn generate_blob_id() -> String {
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(bytes.as_mut());
    hex_encode(bytes.as_ref())
}

/// Mã hóa nội dung blob ảnh của ghi chú khóa; `blob_id` được gắn vào AAD.
pub fn encrypt_locked_blob(
    blob_id: &str,
    key: &ImageBlobKey,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    encrypt_lock_payload(key.as_bytes(), blob_id, LOCK_CONTEXT_BLOB, plaintext)
}

/// Giải mã nội dung blob ảnh đã mã hóa.
pub fn decrypt_locked_blob(
    blob_id: &str,
    key: &ImageBlobKey,
    envelope: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    decrypt_lock_payload(key.as_bytes(), blob_id, LOCK_CONTEXT_BLOB, envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE_ID: &str = "01990000-0000-7000-8000-000000000001";

    #[test]
    fn wrap_unwrap_roundtrip() {
        let nlk = generate_note_lock_key().unwrap();
        let metadata =
            wrap_note_lock_key(NOTE_ID, b"mat-khau-khoa", &nlk, &KdfParams::default(), None, 1, 1)
                .unwrap();
        assert_eq!(metadata.version, LOCK_METADATA_VERSION);
        let unlocked =
            unwrap_note_lock_key(NOTE_ID, b"mat-khau-khoa", &metadata).unwrap();
        assert_eq!(unlocked.as_bytes(), nlk.as_bytes());
    }

    #[test]
    fn wrong_passphrase_fails() {
        let nlk = generate_note_lock_key().unwrap();
        let metadata =
            wrap_note_lock_key(NOTE_ID, b"mat-khau-khoa", &nlk, &KdfParams::default(), None, 1, 1)
                .unwrap();
        let error = unwrap_note_lock_key(NOTE_ID, b"sai-roi", &metadata).unwrap_err();
        assert!(matches!(error, CryptoError::AuthenticationFailed));
    }

    #[test]
    fn metadata_is_bound_to_note_id() {
        let nlk = generate_note_lock_key().unwrap();
        let metadata =
            wrap_note_lock_key(NOTE_ID, b"mat-khau-khoa", &nlk, &KdfParams::default(), None, 1, 1)
                .unwrap();
        let other = "01990000-0000-7000-8000-000000000002";
        let error = unwrap_note_lock_key(other, b"mat-khau-khoa", &metadata).unwrap_err();
        assert!(matches!(error, CryptoError::AuthenticationFailed));
    }

    #[test]
    fn weak_kdf_params_are_rejected() {
        let nlk = generate_note_lock_key().unwrap();
        let params = KdfParams {
            alg: "pbkdf2".to_string(),
            m_kib: 1024,
            t: 1,
            p: 1,
        };
        let error = wrap_note_lock_key(NOTE_ID, b"x", &nlk, &params, None, 1, 1).unwrap_err();
        assert!(matches!(error, CryptoError::InvalidKdfParams));
    }

    #[test]
    fn payload_roundtrip_and_tamper() {
        let key = generate_image_blob_key().unwrap();
        let payload = b"trang thai crdt".repeat(50);
        let envelope =
            encrypt_lock_payload(key.as_bytes(), NOTE_ID, LOCK_CONTEXT_STATE, &payload).unwrap();
        let decrypted =
            decrypt_lock_payload(key.as_bytes(), NOTE_ID, LOCK_CONTEXT_STATE, &envelope).unwrap();
        assert_eq!(decrypted, payload);
        let mut tampered = envelope.clone();
        tampered[10] ^= 0x01;
        let error =
            decrypt_lock_payload(key.as_bytes(), NOTE_ID, LOCK_CONTEXT_STATE, &tampered)
                .unwrap_err();
        assert!(matches!(error, CryptoError::AuthenticationFailed));
    }

    #[test]
    fn blob_roundtrip() {
        let key = generate_image_blob_key().unwrap();
        let blob_id = generate_blob_id();
        assert_eq!(blob_id.len(), 64);
        let plaintext = vec![7u8; 1024];
        let ciphertext = encrypt_locked_blob(&blob_id, &key, &plaintext).unwrap();
        let decrypted = decrypt_locked_blob(&blob_id, &key, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
        let wrapped = wrap_image_blob_key(NOTE_ID, &generate_note_lock_key().unwrap(), &key).unwrap();
        let restored = unwrap_image_blob_key(NOTE_ID, &generate_note_lock_key().unwrap(), &wrapped);
        assert!(restored.is_err(), "khóa NLK khác phải không mở được blob key");
    }
}
