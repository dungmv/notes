pub mod envelope;
pub mod keys;
pub mod notelock;
pub mod recovery;
pub mod stream;

pub use envelope::{
    AadContext, CIPHER_XCHACHA20_POLY1305, CRYPTO_VERSION_V1, ENVELOPE_OVERHEAD,
    decrypt_envelope, encrypt_envelope, pad_plaintext, unpad_plaintext,
};
pub use keys::{
    AccountMasterKey, AuthKey, BlobEncryptionKey, BlobIndexKey, DataEncryptionKey,
    EntityWrapKey, ImageBlobKey, KdfParams, KeyEncryptionKey, MasterSecret, NoteLockKey,
    RecoveryKek, SecretKey32, derive_master_secret, random_key32, random_salt32,
};
pub use recovery::{decode_recovery_key, encode_recovery_key};
pub use stream::{
    BLOB_HEADER_LEN, BLOB_MAGIC, CHUNK_SIZE, decrypt_blob_stream, derive_blob_base_nonce,
    derive_blob_id, derive_blob_id_from_hash, derive_blob_key, encrypt_blob_stream,
};

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Xác thực mật mã thất bại")]
    AuthenticationFailed,
    #[error("Định dạng dữ liệu mã hóa không hợp lệ")]
    InvalidFormat,
    #[error("Phiên bản hoặc thuật toán mã hóa không được hỗ trợ")]
    UnsupportedVersion,
    #[error("Tham số KDF không hợp lệ")]
    InvalidKdfParams,
    #[error("Khóa khôi phục không hợp lệ hoặc sai checksum")]
    InvalidRecoveryKey,
    #[error("Lỗi I/O khi xử lý luồng mã hóa: {0}")]
    Io(#[from] std::io::Error),
}
