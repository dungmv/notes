use notes_crypto::{
    AadContext, AccountMasterKey, CryptoError, KdfParams, RecoveryKek, SecretKey32,
    decode_recovery_key, decrypt_blob_stream, decrypt_envelope, derive_blob_base_nonce,
    derive_blob_id, derive_blob_key, derive_master_secret, encode_recovery_key,
    encrypt_blob_stream, encrypt_envelope, pad_plaintext, random_key32, unpad_plaintext,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use uuid::Uuid;

fn sample_aad() -> AadContext<'static> {
    AadContext {
        context: "notes-sync-delta-v1",
        key_version: 1,
        user_id: Uuid::nil(),
        entity_type: "note",
        entity_id: Uuid::from_u128(42),
        client_op_id: Uuid::from_u128(99),
    }
}

#[test]
fn kat_hkdf_and_argon2id_are_deterministic() {
    let params = KdfParams {
        alg: "argon2id".to_string(),
        m_kib: 1024,
        t: 1,
        p: 1,
    };
    let salt = [0x11u8; 32];
    let ms1 = derive_master_secret(b"correct-horse-battery-staple", &salt, &params).unwrap();
    let ms2 = derive_master_secret(b"correct-horse-battery-staple", &salt, &params).unwrap();
    assert_eq!(ms1.as_bytes(), ms2.as_bytes());

    let ak1 = ms1.derive_auth_key().unwrap();
    let kek1 = ms1.derive_kek().unwrap();
    assert_ne!(ak1.as_bytes(), kek1.as_bytes());
    assert_ne!(ak1.as_bytes(), ms1.as_bytes());
}

#[test]
fn envelope_roundtrip_and_padding_buckets() {
    let key = [0x42u8; 32];
    let aad = sample_aad();

    for size in [0usize, 1, 252, 253, 1020, 1021, 65_536] {
        let plain: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let padded = pad_plaintext(&plain).unwrap();
        if size + 4 <= 256 {
            assert_eq!(padded.len(), 256);
        } else if size + 4 <= 1024 {
            assert_eq!(padded.len() % 256, 0);
        } else {
            assert_eq!(padded.len() % 1024, 0);
        }
        assert_eq!(unpad_plaintext(&padded).unwrap(), plain);

        let env = encrypt_envelope(&key, &plain, &aad).unwrap();
        let dec = decrypt_envelope(&key, &env, &aad).unwrap();
        assert_eq!(dec, plain);
    }
}

#[test]
fn envelope_tamper_and_aad_binding_reject() {
    let key = [0x55u8; 32];
    let aad = sample_aad();
    let env = encrypt_envelope(&key, b"secret note body", &aad).unwrap();

    // Flip bit in ciphertext/tag/nonce
    for idx in [6, env.len() / 2, env.len() - 1] {
        let mut tampered = env.clone();
        tampered[idx] ^= 0x01;
        assert!(matches!(
            decrypt_envelope(&key, &tampered, &aad),
            Err(CryptoError::AuthenticationFailed)
        ));
    }

    // Mutate AAD fields
    let mut bad = aad;
    bad.user_id = Uuid::from_u128(1);
    assert!(matches!(
        decrypt_envelope(&key, &env, &bad),
        Err(CryptoError::AuthenticationFailed)
    ));

    let mut bad = aad;
    bad.entity_id = Uuid::from_u128(2);
    assert!(matches!(
        decrypt_envelope(&key, &env, &bad),
        Err(CryptoError::AuthenticationFailed)
    ));

    let mut bad = aad;
    bad.entity_type = "folder";
    assert!(matches!(
        decrypt_envelope(&key, &env, &bad),
        Err(CryptoError::AuthenticationFailed)
    ));

    let mut bad = aad;
    bad.client_op_id = Uuid::from_u128(3);
    assert!(matches!(
        decrypt_envelope(&key, &env, &bad),
        Err(CryptoError::AuthenticationFailed)
    ));

    let mut bad = aad;
    bad.key_version = 2;
    assert!(matches!(
        decrypt_envelope(&key, &env, &bad),
        Err(CryptoError::AuthenticationFailed)
    ));
}

#[test]
fn stream_blob_roundtrip_and_integrity() {
    let amk = AccountMasterKey::generate();
    let other_amk = AccountMasterKey::generate();

    for len in [0usize, 17, 65_536, 65_536 + 1, 150_000] {
        let plain: Vec<u8> = (0..len).map(|i| ((i * 7) % 256) as u8).collect();
        let hash = Sha256::digest(&plain);

        let bik1 = amk.derive_blob_index_key().unwrap();
        let bik2 = other_amk.derive_blob_index_key().unwrap();
        assert_ne!(derive_blob_id(&bik1, &plain), derive_blob_id(&bik2, &plain));

        let bek = derive_blob_key(&amk, hash.as_slice()).unwrap();
        let base_nonce = derive_blob_base_nonce(&bek).unwrap();

        let mut ct = Vec::new();
        encrypt_blob_stream(&bek, &base_nonce, Cursor::new(&plain), &mut ct).unwrap();

        let mut dec = Vec::new();
        decrypt_blob_stream(&bek, Cursor::new(&ct), &mut dec).unwrap();
        assert_eq!(dec, plain);

        // Truncation fails
        let mut truncated = ct.clone();
        truncated.pop();
        let mut out = Vec::new();
        assert!(decrypt_blob_stream(&bek, Cursor::new(&truncated), &mut out).is_err());
    }

    // Multi-chunk reordering & duplication detection
    let multi: Vec<u8> = vec![0xAB; 65_536 * 2 + 123];
    let hash = Sha256::digest(&multi);
    let bek = derive_blob_key(&amk, hash.as_slice()).unwrap();
    let base_nonce = derive_blob_base_nonce(&bek).unwrap();
    let mut ct = Vec::new();
    encrypt_blob_stream(&bek, &base_nonce, Cursor::new(&multi), &mut ct).unwrap();

    let chunk_ct_len = 65_536 + 16;
    let mut reordered = ct.clone();
    let (c0, rest) = reordered[32..].split_at_mut(chunk_ct_len);
    c0.swap_with_slice(&mut rest[..chunk_ct_len]);
    let mut out = Vec::new();
    assert!(matches!(
        decrypt_blob_stream(&bek, Cursor::new(&reordered), &mut out),
        Err(CryptoError::AuthenticationFailed)
    ));
}

#[test]
fn recovery_key_encode_decode_and_amk_wrap() {
    let rk = random_key32();
    let encoded = encode_recovery_key(&rk);
    let decoded = decode_recovery_key(&encoded).unwrap();
    assert_eq!(rk.as_bytes(), decoded.as_bytes());

    let mut bad = encoded.clone();
    let last = bad.pop().unwrap();
    bad.push(if last == 'A' { 'B' } else { 'A' });
    assert!(matches!(
        decode_recovery_key(&bad),
        Err(CryptoError::InvalidRecoveryKey)
    ));

    let rkek = RecoveryKek::from_recovery_key(&decoded).unwrap();
    let amk = AccountMasterKey::generate();
    let aad = AadContext {
        context: "notes-amk-wrap-v1",
        key_version: 1,
        user_id: Uuid::from_u128(7),
        entity_type: "amk",
        entity_id: Uuid::from_u128(7),
        client_op_id: Uuid::nil(),
    };
    let wrapped = encrypt_envelope(rkek.as_bytes(), amk.as_bytes(), &aad).unwrap();
    let unwrapped = decrypt_envelope(rkek.as_bytes(), &wrapped, &aad).unwrap();
    assert_eq!(unwrapped.as_slice(), amk.as_bytes());
}

#[test]
fn secret_keys_redact_debug_output() {
    let key = SecretKey32::from_bytes([0x99u8; 32]);
    let amk = AccountMasterKey::from_bytes([0xAAu8; 32]);
    assert_eq!(format!("{key:?}"), "[REDACTED]");
    assert_eq!(format!("{amk:?}"), "AccountMasterKey([REDACTED])");
}
