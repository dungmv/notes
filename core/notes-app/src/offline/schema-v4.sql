CREATE TABLE sync_config (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK(enabled IN (0, 1)),
    server_url TEXT NOT NULL DEFAULT '',
    email TEXT NOT NULL DEFAULT '',
    user_id TEXT NOT NULL DEFAULT '',
    device_id TEXT NOT NULL,
    key_version INTEGER NOT NULL DEFAULT 1,
    last_server_cursor INTEGER NOT NULL DEFAULT 0,
    last_synced_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE sync_outbox (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    client_op_id TEXT NOT NULL UNIQUE,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    crypto_version INTEGER,
    key_version INTEGER,
    encrypted_payload BLOB,
    blob_hashes_json TEXT NOT NULL DEFAULT '[]',
    server_seq INTEGER,
    acked INTEGER NOT NULL DEFAULT 0 CHECK(acked IN (0, 1)),
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_sync_outbox_pending ON sync_outbox(acked, seq) WHERE acked = 0;

CREATE TABLE sync_blobs (
    plaintext_hash TEXT PRIMARY KEY,
    blob_id TEXT,
    uploaded INTEGER NOT NULL DEFAULT 0 CHECK(uploaded IN (0, 1))
);

CREATE TABLE sync_lww (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    field TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    client_op_id TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id, field)
);
