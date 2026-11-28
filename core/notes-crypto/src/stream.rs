use crate::CryptoError;
use crate::envelope::{CIPHER_XCHACHA20_POLY1305, CRYPTO_VERSION_V1, TAG_LEN};
use crate::keys::{AccountMasterKey, BlobEncryptionKey, BlobIndexKey};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use zeroize::Zeroizing;

pub const BLOB_MAGIC: [u8; 4] = *b"NTBL";
pub const BLOB_HEADER_LEN: usize = 32;
pub const CHUNK_SIZE_LOG2: u8 = 16;
pub const CHUNK_SIZE: usize = 1 << CHUNK_SIZE_LOG2; // 64 KiB
pub const BASE_NONCE_LEN: usize = 19;

type HmacSha256 = Hmac<Sha256>;

pub fn derive_blob_id(bik: &BlobIndexKey, plaintext: &[u8]) -> [u8; 32] {
    let plaintext_hash = Sha256::digest(plaintext);
    derive_blob_id_from_hash(bik, plaintext_hash.as_slice())
}

pub fn derive_blob_id_from_hash(bik: &BlobIndexKey, plaintext_sha256: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(bik.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(plaintext_sha256);
    mac.finalize().into_bytes().into()
}

pub fn derive_blob_key(
    amk: &AccountMasterKey,
    plaintext_sha256: &[u8],
) -> Result<BlobEncryptionKey, CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(plaintext_sha256), amk.as_bytes());
    let mut okm = Zeroizing::new([0u8; 32]);
    hk.expand(b"notes-e2ee-v1:blob-key", okm.as_mut())
        .map_err(|_| CryptoError::InvalidFormat)?;
    Ok(BlobEncryptionKey::from_bytes(*okm))
}

pub fn derive_blob_base_nonce(bek: &BlobEncryptionKey) -> Result<[u8; BASE_NONCE_LEN], CryptoError> {
    let hk = Hkdf::<Sha256>::new(None, bek.as_bytes());
    let mut nonce = [0u8; BASE_NONCE_LEN];
    hk.expand(b"notes-e2ee-v1:blob-nonce", &mut nonce)
        .map_err(|_| CryptoError::InvalidFormat)?;
    Ok(nonce)
}

fn build_header(base_nonce: &[u8; BASE_NONCE_LEN]) -> [u8; BLOB_HEADER_LEN] {
    let mut header = [0u8; BLOB_HEADER_LEN];
    header[0..4].copy_from_slice(&BLOB_MAGIC);
    header[4] = CRYPTO_VERSION_V1;
    header[5] = CIPHER_XCHACHA20_POLY1305;
    header[6] = CHUNK_SIZE_LOG2;
    // header[7..13] reserved = 0
    header[13..32].copy_from_slice(base_nonce);
    header
}

fn chunk_nonce_and_aad(
    header: &[u8; BLOB_HEADER_LEN],
    base_nonce: &[u8; BASE_NONCE_LEN],
    index: u32,
    is_final: bool,
) -> ([u8; 24], [u8; BLOB_HEADER_LEN + 5]) {
    let flag: u8 = if is_final { 0x01 } else { 0x00 };
    let idx_be = index.to_be_bytes();
    let mut nonce = [0u8; 24];
    nonce[0..19].copy_from_slice(base_nonce);
    nonce[19..23].copy_from_slice(&idx_be);
    nonce[23] = flag;

    let mut aad = [0u8; BLOB_HEADER_LEN + 5];
    aad[0..BLOB_HEADER_LEN].copy_from_slice(header);
    aad[BLOB_HEADER_LEN..BLOB_HEADER_LEN + 4].copy_from_slice(&idx_be);
    aad[BLOB_HEADER_LEN + 4] = flag;
    (nonce, aad)
}

pub fn encrypt_blob_stream<R: Read, W: Write>(
    bek: &BlobEncryptionKey,
    base_nonce: &[u8; BASE_NONCE_LEN],
    mut reader: R,
    mut writer: W,
) -> Result<u64, CryptoError> {
    let header = build_header(base_nonce);
    writer.write_all(&header)?;
    let mut total_written = BLOB_HEADER_LEN as u64;

    let cipher = XChaCha20Poly1305::new(bek.as_bytes().into());
    let mut buf = vec![0u8; CHUNK_SIZE + 1];
    let mut chunk_idx: u32 = 0;

    let mut filled = read_full(&mut reader, &mut buf[..CHUNK_SIZE])?;
    loop {
        let next = if filled == CHUNK_SIZE {
            read_full(&mut reader, &mut buf[CHUNK_SIZE..CHUNK_SIZE + 1])?
        } else {
            0
        };
        let is_final = next == 0;
        let (nonce, aad) = chunk_nonce_and_aad(&header, base_nonce, chunk_idx, is_final);
        let ct = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &buf[..filled],
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)?;
        writer.write_all(&ct)?;
        total_written += ct.len() as u64;

        if is_final {
            break;
        }
        buf[0] = buf[CHUNK_SIZE];
        let rest = read_full(&mut reader, &mut buf[1..CHUNK_SIZE])?;
        filled = 1 + rest;
        chunk_idx = chunk_idx
            .checked_add(1)
            .ok_or(CryptoError::InvalidFormat)?;
    }

    Ok(total_written)
}

pub fn decrypt_blob_stream<R: Read, W: Write>(
    bek: &BlobEncryptionKey,
    mut reader: R,
    mut writer: W,
) -> Result<u64, CryptoError> {
    let mut header = [0u8; BLOB_HEADER_LEN];
    reader
        .read_exact(&mut header)
        .map_err(|_| CryptoError::InvalidFormat)?;
    if header[0..4] != BLOB_MAGIC {
        return Err(CryptoError::InvalidFormat);
    }
    if header[4] != CRYPTO_VERSION_V1
        || header[5] != CIPHER_XCHACHA20_POLY1305
        || header[6] != CHUNK_SIZE_LOG2
    {
        return Err(CryptoError::UnsupportedVersion);
    }
    let base_nonce: [u8; BASE_NONCE_LEN] = header[13..32].try_into().unwrap();
    let cipher = XChaCha20Poly1305::new(bek.as_bytes().into());

    let max_ct_chunk = CHUNK_SIZE + TAG_LEN;
    let mut buf = vec![0u8; max_ct_chunk + 1];
    let mut chunk_idx: u32 = 0;
    let mut total_plain = 0u64;

    let mut filled = read_full(&mut reader, &mut buf[..max_ct_chunk])?;
    if filled < TAG_LEN {
        return Err(CryptoError::InvalidFormat);
    }

    loop {
        let next = if filled == max_ct_chunk {
            read_full(&mut reader, &mut buf[max_ct_chunk..max_ct_chunk + 1])?
        } else {
            0
        };
        let is_final = next == 0;
        let (nonce, aad) = chunk_nonce_and_aad(&header, &base_nonce, chunk_idx, is_final);
        let pt = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &buf[..filled],
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)?;
        writer.write_all(&pt)?;
        total_plain += pt.len() as u64;

        if is_final {
            break;
        }
        buf[0] = buf[max_ct_chunk];
        let rest = read_full(&mut reader, &mut buf[1..max_ct_chunk])?;
        filled = 1 + rest;
        if filled < TAG_LEN {
            return Err(CryptoError::InvalidFormat);
        }
        chunk_idx = chunk_idx
            .checked_add(1)
            .ok_or(CryptoError::InvalidFormat)?;
    }

    Ok(total_plain)
}

fn read_full<R: Read>(reader: &mut R, mut dst: &mut [u8]) -> Result<usize, std::io::Error> {
    let mut total = 0;
    while !dst.is_empty() {
        match reader.read(dst) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                dst = &mut dst[n..];
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(total)
}
