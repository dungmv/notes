use crate::CryptoError;
use crate::keys::SecretKey32;
use sha2::{Digest, Sha256};

const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn checksum2(key: &[u8; 32]) -> [u8; 2] {
    let hash = Sha256::digest(key);
    [hash[0], hash[1]]
}

pub fn encode_recovery_key(key: &SecretKey32) -> String {
    let mut payload = [0u8; 34];
    payload[0..32].copy_from_slice(key.as_bytes());
    let csum = checksum2(key.as_bytes());
    payload[32..34].copy_from_slice(&csum);

    let mut chars = Vec::with_capacity(55);
    let mut buffer: u32 = 0;
    let mut bits: u8 = 0;
    for byte in payload {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1f) as usize;
            chars.push(ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
        chars.push(ALPHABET[idx] as char);
    }

    let mut out = String::with_capacity(65);
    for (i, ch) in chars.into_iter().enumerate() {
        if i > 0 && i % 5 == 0 {
            out.push('-');
        }
        out.push(ch);
    }
    out
}

pub fn decode_recovery_key(input: &str) -> Result<SecretKey32, CryptoError> {
    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'-')
        .map(|b| b.to_ascii_uppercase())
        .collect();
    if cleaned.len() != 55 {
        return Err(CryptoError::InvalidRecoveryKey);
    }

    let mut out = [0u8; 34];
    let mut out_idx = 0usize;
    let mut buffer: u32 = 0;
    let mut bits: u8 = 0;

    for ch in cleaned {
        let val = ALPHABET
            .iter()
            .position(|&c| c == ch)
            .ok_or(CryptoError::InvalidRecoveryKey)? as u32;
        buffer = (buffer << 5) | val;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            if out_idx < 34 {
                out[out_idx] = ((buffer >> bits) & 0xff) as u8;
                out_idx += 1;
            }
        }
    }
    if out_idx != 34 || bits >= 5 || (buffer & ((1 << bits) - 1)) != 0 {
        return Err(CryptoError::InvalidRecoveryKey);
    }

    let key_bytes: [u8; 32] = out[0..32].try_into().unwrap();
    if out[32..34] != checksum2(&key_bytes) {
        return Err(CryptoError::InvalidRecoveryKey);
    }
    Ok(SecretKey32::from_bytes(key_bytes))
}
