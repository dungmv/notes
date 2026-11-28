CREATE TABLE users (
    id UUID PRIMARY KEY,
    email_hash TEXT UNIQUE NOT NULL,
    auth_verifier TEXT NOT NULL,
    kdf_salt BYTEA NOT NULL,
    kdf_params_json JSONB NOT NULL,
    wrapped_amk BYTEA NOT NULL,
    recovery_wrapped_amk BYTEA,
    recovery_auth_verifier TEXT,
    key_ring_encrypted BYTEA,
    key_version INT NOT NULL DEFAULT 1,
    last_sync_seq BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL
);

CREATE TABLE devices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,
    last_seen_at BIGINT NOT NULL,
    revoked_at BIGINT,
    created_at BIGINT NOT NULL
);
CREATE INDEX idx_devices_user ON devices(user_id);

CREATE TABLE refresh_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    expires_at BIGINT NOT NULL,
    revoked_at BIGINT,
    created_at BIGINT NOT NULL
);
CREATE INDEX idx_refresh_tokens_device ON refresh_tokens(device_id);

CREATE TABLE sync_deltas (
    server_seq BIGINT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id UUID NOT NULL,
    client_op_id UUID NOT NULL,
    crypto_version SMALLINT NOT NULL DEFAULT 1,
    key_version INT NOT NULL DEFAULT 1,
    encrypted_payload BYTEA NOT NULL,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, server_seq),
    UNIQUE (user_id, client_op_id)
);
CREATE INDEX idx_sync_deltas_entity ON sync_deltas(user_id, entity_id, server_seq);

CREATE TABLE user_blobs (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    byte_size BIGINT NOT NULL,
    storage_key TEXT NOT NULL,
    ciphertext_sha256 TEXT NOT NULL,
    ref_count INT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, content_hash)
);

CREATE TABLE blob_upload_tokens (
    token TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    byte_size BIGINT NOT NULL,
    ciphertext_sha256 TEXT NOT NULL,
    staging_key TEXT NOT NULL,
    final_key TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    completed_at BIGINT
);
