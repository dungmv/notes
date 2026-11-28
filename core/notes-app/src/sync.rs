use crate::editor::{Editor, encode_state};
use crate::offline::{
    OfflineError, Store, new_uuid_v7, now_ms, reindex_tx, summarize_note_snapshot,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use notes_crypto::{
    AadContext, AccountMasterKey, CRYPTO_VERSION_V1, CryptoError, KdfParams, RecoveryKek,
    SecretKey32, decode_recovery_key, decrypt_blob_stream, decrypt_envelope, derive_blob_base_nonce,
    derive_blob_id_from_hash, derive_blob_key, derive_master_secret, encode_recovery_key,
    encrypt_blob_stream, encrypt_envelope, random_key32, random_salt32,
};
use notes_sync::{
    AuthResponse, ChangeCredentialsPayload, DeviceInfoDto, HttpSyncTransport, LoginPayload,
    PushDeltaDto, RecoverPayload, RegisterPayload, SyncTransport, SyncTransportError,
};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;
use uuid::Uuid;
use yrs::updates::decoder::Decode;
use yrs::{Doc, Transact, Update};

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Offline(#[from] OfflineError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Transport(#[from] SyncTransportError),
    #[error("Đồng bộ chưa được kích hoạt hoặc chưa mở khóa")]
    NotAuthenticated,
    #[error("Dữ liệu đồng bộ không hợp lệ: {0}")]
    InvalidData(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SyncPayload {
    FolderUpsert {
        id: String,
        name: String,
        kind: String,
        sort_order: i64,
        created_at: i64,
        updated_at: i64,
    },
    NoteCreate {
        id: String,
        folder_id: String,
        crdt_state_b64: String,
        created_at: i64,
        updated_at: i64,
    },
    NoteCrdtUpdate {
        id: String,
        crdt_state_b64: String,
        updated_at: i64,
        #[serde(default)]
        attachments: Vec<SyncAttachmentMeta>,
    },
    NoteMove {
        id: String,
        folder_id: String,
        updated_at: i64,
    },
    NotePin {
        id: String,
        is_pinned: bool,
        updated_at: i64,
    },
    NoteDelete {
        id: String,
        deleted_at: Option<i64>,
        updated_at: i64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncAttachmentMeta {
    pub id: String,
    pub kind: String,
    pub file_name: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub plaintext_hash: String,
    pub blob_id: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncStatus {
    pub enabled: bool,
    pub authenticated: bool,
    pub unlocked: bool,
    pub server_url: String,
    pub email: String,
    pub user_id: String,
    pub device_id: String,
    pub key_version: u32,
    pub last_server_cursor: i64,
    pub last_synced_at: i64,
    pub pending_outbox_count: i64,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuthSuccess {
    pub amk_b64: String,
    pub refresh_token: String,
    pub recovery_key: Option<String>,
    pub status: SyncStatus,
}

#[derive(Clone, Debug, Serialize)]
pub struct SyncRunResult {
    pub pushed: usize,
    pub pulled: usize,
    pub empty_pull: bool,
    pub duration_ms: u64,
    pub changed_note_ids: Vec<String>,
    pub folders_changed: bool,
    pub notes_list_changed: bool,
    pub status: SyncStatus,
}

pub struct SyncClient {
    amk: Option<AccountMasterKey>,
    access_token: Option<String>,
    refresh_token: Option<String>,
    last_error: Option<String>,
    transport_override: Option<Arc<dyn SyncTransport>>,
}

impl Default for SyncClient {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncClient {
    pub fn new() -> Self {
        Self {
            amk: None,
            access_token: None,
            refresh_token: None,
            last_error: None,
            transport_override: None,
        }
    }

    pub fn with_transport(transport: Arc<dyn SyncTransport>) -> Self {
        Self {
            amk: None,
            access_token: None,
            refresh_token: None,
            last_error: None,
            transport_override: Some(transport),
        }
    }

    fn transport(&self, server_url: &str) -> Result<Arc<dyn SyncTransport>, SyncError> {
        if let Some(t) = &self.transport_override {
            return Ok(Arc::clone(t));
        }
        if server_url.trim().is_empty() {
            return Err(SyncError::NotAuthenticated);
        }
        Ok(Arc::new(HttpSyncTransport::new(server_url)))
    }

    pub fn status(&self, store: &Store) -> Result<SyncStatus, SyncError> {
        let (enabled, server_url, email, user_id, device_id, key_version, cursor, synced_at): (
            bool,
            String,
            String,
            String,
            String,
            u32,
            i64,
            i64,
        ) = store.db().query_row(
            "SELECT enabled, server_url, email, user_id, device_id, key_version, last_server_cursor, last_synced_at
             FROM sync_config WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)? != 0,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        ).map_err(OfflineError::from)?;

        let pending: i64 = store
            .db()
            .query_row(
                "SELECT COUNT(*) FROM sync_outbox WHERE acked = 0",
                [],
                |row| row.get(0),
            )
            .map_err(OfflineError::from)?;

        Ok(SyncStatus {
            enabled,
            authenticated: self.refresh_token.is_some(),
            unlocked: self.amk.is_some(),
            server_url,
            email,
            user_id,
            device_id,
            key_version,
            last_server_cursor: cursor,
            last_synced_at: synced_at,
            pending_outbox_count: pending,
            last_error: self.last_error.clone(),
        })
    }

    pub fn unlock_saved_session(
        &mut self,
        store: &Store,
        amk_b64: &str,
        refresh_token: &str,
    ) -> Result<SyncStatus, SyncError> {
        let amk_bytes = B64
            .decode(amk_b64)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let amk = AccountMasterKey::from_slice(&amk_bytes)?;
        self.amk = Some(amk);
        self.refresh_token = Some(refresh_token.to_string());
        self.access_token = None;
        self.last_error = None;
        self.status(store)
    }

    pub fn clear_session(&mut self, store: &mut Store) -> Result<SyncStatus, SyncError> {
        self.amk = None;
        self.access_token = None;
        self.refresh_token = None;
        self.last_error = None;
        store
            .db()
            .execute("UPDATE sync_config SET enabled = 0 WHERE id = 1", [])
            .map_err(OfflineError::from)?;
        self.status(store)
    }

    pub fn register(
        &mut self,
        store: &mut Store,
        server_url: &str,
        email: &str,
        passphrase: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<AuthSuccess, SyncError> {
        let transport = self.transport(server_url)?;
        let device_id = read_device_id(store)?;
        let kdf_salt = random_salt32();
        let kdf_params = KdfParams::default();
        let ms = derive_master_secret(passphrase.as_bytes(), &kdf_salt, &kdf_params)?;
        let ak = ms.derive_auth_key()?;
        let kek = ms.derive_kek()?;
        let amk = AccountMasterKey::generate();

        let wrapped_amk = encrypt_envelope(
            kek.as_bytes(),
            amk.as_bytes(),
            &AadContext {
                context: "notes-amk-wrap-v1",
                key_version: 1,
                user_id: Uuid::nil(),
                entity_type: "amk",
                entity_id: Uuid::nil(),
                client_op_id: Uuid::nil(),
            },
        )?;

        let recovery_key = random_key32();
        let recovery_display = encode_recovery_key(&recovery_key);
        let recovery_kek = RecoveryKek::from_recovery_key(&recovery_key)?;
        let recovery_wrapped = encrypt_envelope(
            recovery_kek.as_bytes(),
            amk.as_bytes(),
            &AadContext {
                context: "notes-amk-wrap-v1",
                key_version: 1,
                user_id: Uuid::nil(),
                entity_type: "recovery_amk",
                entity_id: Uuid::nil(),
                client_op_id: Uuid::nil(),
            },
        )?;
        let recovery_auth = derive_recovery_auth_key(&recovery_key);

        let resp = transport.register(&RegisterPayload {
            email: email.trim().to_string(),
            auth_key_b64: B64.encode(ak.as_bytes()),
            kdf_salt_b64: B64.encode(kdf_salt),
            kdf_params,
            wrapped_amk_b64: B64.encode(wrapped_amk),
            recovery_wrapped_amk_b64: Some(B64.encode(recovery_wrapped)),
            recovery_auth_key_b64: Some(B64.encode(recovery_auth)),
            device_id,
            device_name: device_name.to_string(),
            platform: platform.to_string(),
        })?;

        self.finish_auth(store, server_url, email, amk, resp, Some(recovery_display))
    }

    pub fn login(
        &mut self,
        store: &mut Store,
        server_url: &str,
        email: &str,
        passphrase: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<AuthSuccess, SyncError> {
        let transport = self.transport(server_url)?;
        let device_id = read_device_id(store)?;
        let salt_resp = transport.get_salt(email.trim())?;
        let salt_bytes = B64
            .decode(&salt_resp.kdf_salt_b64)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;

        let ms = derive_master_secret(passphrase.as_bytes(), &salt_bytes, &salt_resp.kdf_params)?;
        let ak = ms.derive_auth_key()?;
        let kek = ms.derive_kek()?;

        let resp = transport.login(&LoginPayload {
            email: email.trim().to_string(),
            auth_key_b64: B64.encode(ak.as_bytes()),
            device_id,
            device_name: device_name.to_string(),
            platform: platform.to_string(),
        })?;

        let wrapped = B64
            .decode(&resp.key_bundle.wrapped_amk_b64)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let amk_bytes = decrypt_envelope(
            kek.as_bytes(),
            &wrapped,
            &AadContext {
                context: "notes-amk-wrap-v1",
                key_version: resp.key_bundle.key_version,
                user_id: Uuid::nil(),
                entity_type: "amk",
                entity_id: Uuid::nil(),
                client_op_id: Uuid::nil(),
            },
        )?;
        let amk = AccountMasterKey::from_slice(&amk_bytes)?;
        self.finish_auth(store, server_url, email, amk, resp, None)
    }

    pub fn change_password(
        &mut self,
        store: &Store,
        current_passphrase: &str,
        new_passphrase: &str,
    ) -> Result<(), SyncError> {
        let status = self.status(store)?;
        let amk = self.amk.clone().ok_or(SyncError::NotAuthenticated)?;
        let transport = self.transport(&status.server_url)?;
        let device_id = Uuid::parse_str(&status.device_id)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;

        let cur_salt_resp = transport.get_salt(&status.email)?;
        let cur_salt = B64
            .decode(&cur_salt_resp.kdf_salt_b64)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let cur_ms = derive_master_secret(
            current_passphrase.as_bytes(),
            &cur_salt,
            &cur_salt_resp.kdf_params,
        )?;
        let cur_ak = cur_ms.derive_auth_key()?;

        let new_salt = random_salt32();
        let new_params = KdfParams::default();
        let new_ms = derive_master_secret(new_passphrase.as_bytes(), &new_salt, &new_params)?;
        let new_ak = new_ms.derive_auth_key()?;
        let new_kek = new_ms.derive_kek()?;
        let new_wrapped = encrypt_envelope(
            new_kek.as_bytes(),
            amk.as_bytes(),
            &AadContext {
                context: "notes-amk-wrap-v1",
                key_version: status.key_version,
                user_id: Uuid::nil(),
                entity_type: "amk",
                entity_id: Uuid::nil(),
                client_op_id: Uuid::nil(),
            },
        )?;

        let payload = ChangeCredentialsPayload {
            current_auth_key_b64: B64.encode(cur_ak.as_bytes()),
            new_auth_key_b64: B64.encode(new_ak.as_bytes()),
            new_kdf_salt_b64: B64.encode(new_salt),
            new_kdf_params: new_params,
            new_wrapped_amk_b64: B64.encode(new_wrapped),
        };

        self.with_token(&transport, device_id, |token| {
            transport.change_credentials(token, device_id, &payload)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn recover_account(
        &mut self,
        store: &mut Store,
        server_url: &str,
        email: &str,
        recovery_key_str: &str,
        new_passphrase: &str,
        device_name: &str,
        platform: &str,
    ) -> Result<AuthSuccess, SyncError> {
        let transport = self.transport(server_url)?;
        let device_id = read_device_id(store)?;
        let rk = decode_recovery_key(recovery_key_str)?;
        let recovery_auth_b64 = B64.encode(derive_recovery_auth_key(&rk));
        let bundle = transport.recover_bundle(email.trim(), &recovery_auth_b64)?;
        let recovery_kek = RecoveryKek::from_recovery_key(&rk)?;
        let wrapped_bytes = B64
            .decode(&bundle.recovery_wrapped_amk_b64)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let amk_bytes = decrypt_envelope(
            recovery_kek.as_bytes(),
            &wrapped_bytes,
            &AadContext {
                context: "notes-amk-wrap-v1",
                key_version: bundle.key_version,
                user_id: Uuid::nil(),
                entity_type: "recovery_amk",
                entity_id: Uuid::nil(),
                client_op_id: Uuid::nil(),
            },
        )?;
        let amk = AccountMasterKey::from_slice(&amk_bytes)?;

        let new_salt = random_salt32();
        let new_params = KdfParams::default();
        let new_ms = derive_master_secret(new_passphrase.as_bytes(), &new_salt, &new_params)?;
        let new_ak = new_ms.derive_auth_key()?;
        let new_kek = new_ms.derive_kek()?;
        let new_wrapped = encrypt_envelope(
            new_kek.as_bytes(),
            amk.as_bytes(),
            &AadContext {
                context: "notes-amk-wrap-v1",
                key_version: bundle.key_version,
                user_id: Uuid::nil(),
                entity_type: "amk",
                entity_id: Uuid::nil(),
                client_op_id: Uuid::nil(),
            },
        )?;

        let resp = transport.recover(&RecoverPayload {
            email: email.trim().to_string(),
            recovery_auth_key_b64: recovery_auth_b64,
            new_auth_key_b64: B64.encode(new_ak.as_bytes()),
            new_kdf_salt_b64: B64.encode(new_salt),
            new_kdf_params: new_params,
            new_wrapped_amk_b64: B64.encode(new_wrapped),
            device_id,
            device_name: device_name.to_string(),
            platform: platform.to_string(),
        })?;

        self.finish_auth(store, server_url, email, amk, resp, None)
    }

    pub fn list_devices(&mut self, store: &Store) -> Result<Vec<DeviceInfoDto>, SyncError> {
        let status = self.status(store)?;
        let transport = self.transport(&status.server_url)?;
        let device_id = Uuid::parse_str(&status.device_id)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let resp = self.with_token(&transport, device_id, |token| {
            transport.list_devices(token, device_id)
        })?;
        Ok(resp.devices)
    }

    pub fn revoke_device(&mut self, store: &Store, target_device_id: Uuid) -> Result<(), SyncError> {
        let status = self.status(store)?;
        let transport = self.transport(&status.server_url)?;
        let device_id = Uuid::parse_str(&status.device_id)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        self.with_token(&transport, device_id, |token| {
            transport.revoke_device(token, device_id, target_device_id)
        })
    }

    pub fn sync_now(&mut self, store: &mut Store) -> Result<SyncRunResult, SyncError> {
        let started = std::time::Instant::now();
        let res = self.sync_now_inner(store, started);
        if let Err(err) = &res {
            self.last_error = Some(err.to_string());
        } else {
            self.last_error = None;
        }
        res
    }

    fn sync_now_inner(
        &mut self,
        store: &mut Store,
        started: std::time::Instant,
    ) -> Result<SyncRunResult, SyncError> {
        let status = self.status(store)?;
        if !status.enabled || !status.unlocked || !status.authenticated {
            return Err(SyncError::NotAuthenticated);
        }
        let amk = self.amk.clone().ok_or(SyncError::NotAuthenticated)?;
        let user_id = Uuid::parse_str(&status.user_id)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let device_id = Uuid::parse_str(&status.device_id)
            .map_err(|e| SyncError::InvalidData(e.to_string()))?;
        let transport = self.transport(&status.server_url)?;

        // 1. Push pending outbox items
        let pushed = self.push_outbox(store, &transport, &amk, user_id, device_id, status.key_version)?;

        // 2. Pull paginated deltas
        let mut cursor = status.last_server_cursor;
        let mut pulled = 0usize;
        let mut changed_note_ids = HashSet::new();
        let mut folders_changed = false;
        let mut notes_list_changed = false;

        loop {
            let page = self.with_token(&transport, device_id, |token| {
                transport.pull(token, device_id, cursor, 100)
            })?;
            if page.deltas.is_empty() {
                break;
            }
            let page_len = page.deltas.len();
            let mut decrypted_batch = Vec::with_capacity(page_len);
            for d in &page.deltas {
                let env = B64
                    .decode(&d.encrypted_payload_b64)
                    .map_err(|e| SyncError::InvalidData(e.to_string()))?;
                let aad = AadContext {
                    context: "notes-sync-delta-v1",
                    key_version: d.key_version,
                    user_id,
                    entity_type: &d.entity_type,
                    entity_id: d.entity_id,
                    client_op_id: d.client_op_id,
                };
                let pt = decrypt_envelope(amk.as_bytes(), &env, &aad)?;
                let payload: SyncPayload = serde_json::from_slice(&pt)
                    .map_err(|e| SyncError::InvalidData(e.to_string()))?;

                if let SyncPayload::NoteCrdtUpdate { attachments, .. } = &payload {
                    for att in attachments {
                        self.ensure_blob_downloaded(store, &transport, &amk, device_id, att)?;
                    }
                }
                decrypted_batch.push((d.server_seq, d.client_op_id.to_string(), payload));
            }

            let tx = store
                .db_mut()
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(OfflineError::from)?;
            for (_, client_op_id, payload) in decrypted_batch {
                apply_pulled_payload(
                    &tx,
                    &client_op_id,
                    payload,
                    &mut changed_note_ids,
                    &mut folders_changed,
                    &mut notes_list_changed,
                )?;
            }
            tx.execute(
                "UPDATE sync_config SET last_server_cursor = ?1, last_synced_at = ?2 WHERE id = 1",
                params![page.next_cursor, now_ms()],
            )
            .map_err(OfflineError::from)?;
            tx.commit().map_err(OfflineError::from)?;

            cursor = page.next_cursor;
            pulled += page_len;
            if !page.has_more {
                break;
            }
        }

        if pulled == 0 {
            store
                .db()
                .execute(
                    "UPDATE sync_config SET last_synced_at = ?1 WHERE id = 1",
                    [now_ms()],
                )
                .map_err(OfflineError::from)?;
        }

        let updated_status = self.status(store)?;
        Ok(SyncRunResult {
            pushed,
            pulled,
            empty_pull: pulled == 0 && pushed == 0,
            duration_ms: started.elapsed().as_millis() as u64,
            changed_note_ids: changed_note_ids.into_iter().collect(),
            folders_changed,
            notes_list_changed,
            status: updated_status,
        })
    }

    fn push_outbox(
        &mut self,
        store: &mut Store,
        transport: &Arc<dyn SyncTransport>,
        amk: &AccountMasterKey,
        user_id: Uuid,
        device_id: Uuid,
        key_version: u32,
    ) -> Result<usize, SyncError> {
        let mut total_pushed = 0usize;
        loop {
            let rows = load_pending_outbox(store, 100)?;
            if rows.is_empty() {
                break;
            }

            let mut deltas = Vec::with_capacity(rows.len());
            for mut row in rows {
                for hash in &row.blob_hashes {
                    self.ensure_blob_uploaded(store, transport, amk, device_id, hash)?;
                }

                let (crypto_ver, kv, env_bytes) = if let Some(existing) = row.encrypted_payload.take() {
                    (
                        row.crypto_version.unwrap_or(CRYPTO_VERSION_V1),
                        row.key_version.unwrap_or(key_version),
                        existing,
                    )
                } else {
                    let op_uuid = Uuid::parse_str(&row.client_op_id)
                        .map_err(|e| SyncError::InvalidData(e.to_string()))?;
                    let entity_uuid = to_entity_uuid(&row.entity_id);
                    let aad = AadContext {
                        context: "notes-sync-delta-v1",
                        key_version,
                        user_id,
                        entity_type: &row.entity_type,
                        entity_id: entity_uuid,
                        client_op_id: op_uuid,
                    };
                    let env = encrypt_envelope(amk.as_bytes(), row.payload_json.as_bytes(), &aad)?;
                    store
                        .db()
                        .execute(
                            "UPDATE sync_outbox SET crypto_version = ?1, key_version = ?2, encrypted_payload = ?3 WHERE seq = ?4",
                            params![i64::from(CRYPTO_VERSION_V1), i64::from(key_version), &env, row.seq],
                        )
                        .map_err(OfflineError::from)?;
                    (CRYPTO_VERSION_V1, key_version, env)
                };

                deltas.push(PushDeltaDto {
                    client_op_id: Uuid::parse_str(&row.client_op_id)
                        .map_err(|e| SyncError::InvalidData(e.to_string()))?,
                    entity_type: row.entity_type,
                    entity_id: to_entity_uuid(&row.entity_id),
                    crypto_version: crypto_ver,
                    key_version: kv,
                    encrypted_payload_b64: B64.encode(&env_bytes),
                });
            }

            let resp = self.with_token(transport, device_id, |token| {
                transport.push(token, device_id, &deltas)
            })?;

            let tx = store
                .db_mut()
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(OfflineError::from)?;
            for receipt in &resp.receipts {
                tx.execute(
                    "UPDATE sync_outbox SET acked = 1, server_seq = ?1 WHERE client_op_id = ?2",
                    params![receipt.server_seq, receipt.client_op_id.to_string()],
                )
                .map_err(OfflineError::from)?;
            }
            tx.commit().map_err(OfflineError::from)?;
            total_pushed += resp.receipts.len();
        }
        Ok(total_pushed)
    }

    fn ensure_blob_uploaded(
        &mut self,
        store: &Store,
        transport: &Arc<dyn SyncTransport>,
        amk: &AccountMasterKey,
        device_id: Uuid,
        plaintext_hash: &str,
    ) -> Result<(), SyncError> {
        let already: Option<bool> = store
            .db()
            .query_row(
                "SELECT uploaded FROM sync_blobs WHERE plaintext_hash = ?1",
                [plaintext_hash],
                |row| row.get::<_, i64>(0).map(|v| v != 0),
            )
            .optional()
            .map_err(OfflineError::from)?;
        if already == Some(true) {
            return Ok(());
        }

        let plain_bytes = store.read_blob_bytes(plaintext_hash)?;
        let hash_bytes = Sha256::digest(&plain_bytes);
        let bik = amk.derive_blob_index_key()?;
        let blob_id = hex_encode(&derive_blob_id_from_hash(&bik, hash_bytes.as_slice()));
        let bek = derive_blob_key(amk, hash_bytes.as_slice())?;
        let base_nonce = derive_blob_base_nonce(&bek)?;

        let mut ct = Vec::new();
        encrypt_blob_stream(&bek, &base_nonce, Cursor::new(&plain_bytes), &mut ct)?;
        let ct_sha256 = hex_encode(Sha256::digest(&ct).as_slice());

        let presign = self.with_token(transport, device_id, |token| {
            transport.presign_upload(token, device_id, &blob_id, ct.len() as u64, &ct_sha256)
        })?;

        if !presign.exists {
            let url = presign
                .upload_url
                .ok_or_else(|| SyncError::InvalidData("Thiếu upload_url".into()))?;
            let upload_token = presign
                .upload_token
                .ok_or_else(|| SyncError::InvalidData("Thiếu upload_token".into()))?;
            transport.upload_presigned(&url, &ct)?;
            self.with_token(transport, device_id, |token| {
                transport.complete_upload(token, device_id, &upload_token)
            })?;
        }

        store
            .db()
            .execute(
                "INSERT INTO sync_blobs(plaintext_hash, blob_id, uploaded) VALUES(?1, ?2, 1)
                 ON CONFLICT(plaintext_hash) DO UPDATE SET blob_id = excluded.blob_id, uploaded = 1",
                params![plaintext_hash, blob_id],
            )
            .map_err(OfflineError::from)?;
        Ok(())
    }

    fn ensure_blob_downloaded(
        &mut self,
        store: &Store,
        transport: &Arc<dyn SyncTransport>,
        amk: &AccountMasterKey,
        device_id: Uuid,
        meta: &SyncAttachmentMeta,
    ) -> Result<(), SyncError> {
        if store.read_blob_bytes(&meta.plaintext_hash).is_ok() {
            return Ok(());
        }
        let hash_bytes = hex_decode(&meta.plaintext_hash)
            .ok_or_else(|| SyncError::InvalidData("content_hash không hợp lệ".into()))?;
        let bik = amk.derive_blob_index_key()?;
        let blob_id = hex_encode(&derive_blob_id_from_hash(&bik, &hash_bytes));
        let presign = self.with_token(transport, device_id, |token| {
            transport.presign_download(token, device_id, &blob_id)
        })?;
        let ct = transport.download_presigned(&presign.download_url)?;
        let bek = derive_blob_key(amk, &hash_bytes)?;
        let mut plain = Vec::new();
        decrypt_blob_stream(&bek, Cursor::new(&ct), &mut plain)?;
        let actual_hash = hex_encode(Sha256::digest(&plain).as_slice());
        if actual_hash != meta.plaintext_hash {
            return Err(CryptoError::AuthenticationFailed.into());
        }
        store.write_blob_bytes(&meta.plaintext_hash, &plain)?;
        Ok(())
    }

    fn with_token<T>(
        &mut self,
        transport: &Arc<dyn SyncTransport>,
        device_id: Uuid,
        mut call: impl FnMut(&str) -> Result<T, SyncTransportError>,
    ) -> Result<T, SyncError> {
        if let Some(token) = &self.access_token {
            match call(token) {
                Ok(v) => return Ok(v),
                Err(SyncTransportError::Unauthorized) => {
                    self.access_token = None;
                }
                Err(e) => return Err(e.into()),
            }
        }
        let refresh = self
            .refresh_token
            .as_deref()
            .ok_or(SyncError::NotAuthenticated)?;
        match transport.refresh(refresh, device_id) {
            Ok(auth) => {
                self.access_token = Some(auth.access_token);
                self.refresh_token = Some(auth.refresh_token);
                let token = self.access_token.as_deref().unwrap();
                Ok(call(token)?)
            }
            Err(SyncTransportError::Unauthorized) => {
                self.access_token = None;
                self.refresh_token = None;
                Err(SyncTransportError::Unauthorized.into())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn finish_auth(
        &mut self,
        store: &mut Store,
        server_url: &str,
        email: &str,
        amk: AccountMasterKey,
        resp: AuthResponse,
        recovery_key: Option<String>,
    ) -> Result<AuthSuccess, SyncError> {
        let was_enabled: bool = store
            .db()
            .query_row("SELECT enabled FROM sync_config WHERE id = 1", [], |row| {
                row.get::<_, i64>(0).map(|v| v != 0)
            })
            .map_err(OfflineError::from)?;

        let amk_b64 = B64.encode(amk.as_bytes());
        self.amk = Some(amk);
        self.access_token = Some(resp.access_token);
        self.refresh_token = Some(resp.refresh_token.clone());
        self.last_error = None;

        let tx = store
            .db_mut()
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(OfflineError::from)?;
        tx.execute(
            "UPDATE sync_config SET enabled = 1, server_url = ?1, email = ?2, user_id = ?3, key_version = ?4 WHERE id = 1",
            params![
                server_url.trim_end_matches('/'),
                email.trim(),
                resp.user_id.to_string(),
                i64::from(resp.key_bundle.key_version)
            ],
        )
        .map_err(OfflineError::from)?;
        if !was_enabled {
            bootstrap_outbox_tx(&tx)?;
        }
        tx.commit().map_err(OfflineError::from)?;

        let status = self.status(store)?;
        Ok(AuthSuccess {
            amk_b64,
            refresh_token: resp.refresh_token,
            recovery_key,
            status,
        })
    }
}

struct OutboxRow {
    seq: i64,
    client_op_id: String,
    entity_type: String,
    entity_id: String,
    payload_json: String,
    crypto_version: Option<u8>,
    key_version: Option<u32>,
    encrypted_payload: Option<Vec<u8>>,
    blob_hashes: Vec<String>,
}

fn load_pending_outbox(store: &Store, limit: i64) -> Result<Vec<OutboxRow>, SyncError> {
    let mut stmt = store
        .db()
        .prepare(
            "SELECT seq, client_op_id, entity_type, entity_id, payload_json,
                    crypto_version, key_version, encrypted_payload, blob_hashes_json
             FROM sync_outbox WHERE acked = 0 ORDER BY seq ASC LIMIT ?1",
        )
        .map_err(OfflineError::from)?;
    let rows = stmt
        .query_map([limit], |row| {
            let blob_json: String = row.get(8)?;
            let blob_hashes: Vec<String> = serde_json::from_str(&blob_json).unwrap_or_default();
            Ok(OutboxRow {
                seq: row.get(0)?,
                client_op_id: row.get(1)?,
                entity_type: row.get(2)?,
                entity_id: row.get(3)?,
                payload_json: row.get(4)?,
                crypto_version: row.get::<_, Option<i64>>(5)?.map(|v| v as u8),
                key_version: row.get::<_, Option<i64>>(6)?.map(|v| v as u32),
                encrypted_payload: row.get(7)?,
                blob_hashes,
            })
        })
        .map_err(OfflineError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(OfflineError::from)?;
    Ok(rows)
}

fn apply_pulled_payload(
    tx: &Transaction<'_>,
    client_op_id: &str,
    payload: SyncPayload,
    changed_notes: &mut HashSet<String>,
    folders_changed: &mut bool,
    notes_list_changed: &mut bool,
) -> Result<(), SyncError> {
    match payload {
        SyncPayload::FolderUpsert {
            id,
            name,
            kind,
            sort_order,
            created_at,
            updated_at,
        } => {
            if lww_accept(tx, "folder", &id, "upsert", updated_at, client_op_id)? {
                tx.execute(
                    "INSERT INTO folders(id, name, kind, sort_order, created_at, updated_at)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at",
                    params![id, name, kind, sort_order, created_at, updated_at],
                )
                .map_err(OfflineError::from)?;
                *folders_changed = true;
            }
        }
        SyncPayload::NoteCreate {
            id,
            folder_id,
            crdt_state_b64,
            created_at,
            updated_at,
        } => {
            let state = B64
                .decode(&crdt_state_b64)
                .map_err(|e| SyncError::InvalidData(e.to_string()))?;
            let exists: bool = tx
                .query_row("SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1)", [&id], |r| r.get(0))
                .map_err(OfflineError::from)?;
            if !exists {
                let target_folder = if folder_exists(tx, &folder_id)? {
                    folder_id
                } else {
                    "inbox".to_string()
                };
                let editor = Editor::from_state(&state).map_err(OfflineError::from)?;
                let (title, snippet, text, checklist, images) =
                    summarize_note_snapshot(editor.snapshot());
                tx.execute(
                    "INSERT INTO notes(id, folder_id, crdt_state, revision, title, snippet,
                        plain_text_cache, has_checklist, has_attachments, created_at, updated_at)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        id,
                        target_folder,
                        state,
                        editor.snapshot().revision as i64,
                        title,
                        snippet,
                        text,
                        checklist,
                        images,
                        created_at,
                        updated_at
                    ],
                )
                .map_err(OfflineError::from)?;
                reindex_tx(tx, &id)?;
                changed_notes.insert(id);
                *notes_list_changed = true;
            }
        }
        SyncPayload::NoteCrdtUpdate {
            id,
            crdt_state_b64,
            updated_at,
            attachments,
        } => {
            let remote_state = B64
                .decode(&crdt_state_b64)
                .map_err(|e| SyncError::InvalidData(e.to_string()))?;
            for att in &attachments {
                tx.execute(
                    "INSERT INTO attachments(id, note_id, kind, file_name, mime_type, byte_size, content_hash, created_at)
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(id) DO NOTHING",
                    params![
                        att.id,
                        id,
                        att.kind,
                        att.file_name,
                        att.mime_type,
                        att.byte_size,
                        att.plaintext_hash,
                        att.created_at
                    ],
                )
                .map_err(OfflineError::from)?;
            }

            let existing: Option<(Vec<u8>, i64)> = tx
                .query_row(
                    "SELECT crdt_state, revision FROM notes WHERE id = ?1",
                    [&id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(OfflineError::from)?;

            if let Some((local_state, _local_rev)) = existing {
                let doc = Doc::new();
                {
                    let mut txn = doc.transact_mut();
                    let u_local = Update::decode_v1(&local_state)
                        .map_err(|e| SyncError::InvalidData(e.to_string()))?;
                    txn.apply_update(u_local)
                        .map_err(|e| SyncError::InvalidData(e.to_string()))?;
                    let u_remote = Update::decode_v1(&remote_state)
                        .map_err(|e| SyncError::InvalidData(e.to_string()))?;
                    txn.apply_update(u_remote)
                        .map_err(|e| SyncError::InvalidData(e.to_string()))?;
                }
                let merged_bytes = encode_state(&doc);
                let editor = Editor::from_state(&merged_bytes).map_err(OfflineError::from)?;
                let next_rev = editor.snapshot().revision as i64;
                let (title, snippet, text, checklist, images) =
                    summarize_note_snapshot(editor.snapshot());
                tx.execute(
                    "UPDATE notes SET crdt_state = ?1, revision = ?2, title = ?3, snippet = ?4,
                     plain_text_cache = ?5, has_checklist = ?6, has_attachments = ?7, updated_at = ?8
                     WHERE id = ?9",
                    params![
                        merged_bytes,
                        next_rev,
                        title,
                        snippet,
                        text,
                        checklist,
                        images,
                        updated_at,
                        id
                    ],
                )
                .map_err(OfflineError::from)?;
            } else {
                let editor = Editor::from_state(&remote_state).map_err(OfflineError::from)?;
                let (title, snippet, text, checklist, images) =
                    summarize_note_snapshot(editor.snapshot());
                tx.execute(
                    "INSERT INTO notes(id, folder_id, crdt_state, revision, title, snippet,
                        plain_text_cache, has_checklist, has_attachments, created_at, updated_at)
                     VALUES(?1, 'inbox', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
                    params![
                        id,
                        remote_state,
                        editor.snapshot().revision as i64,
                        title,
                        snippet,
                        text,
                        checklist,
                        images,
                        updated_at
                    ],
                )
                .map_err(OfflineError::from)?;
            }
            reindex_tx(tx, &id)?;
            changed_notes.insert(id);
            *notes_list_changed = true;
        }
        SyncPayload::NoteMove {
            id,
            folder_id,
            updated_at,
        } => {
            if lww_accept(tx, "note", &id, "folder_id", updated_at, client_op_id)? {
                let target = if folder_exists(tx, &folder_id)? {
                    folder_id
                } else {
                    "inbox".to_string()
                };
                tx.execute(
                    "UPDATE notes SET folder_id = ?1, updated_at = ?2 WHERE id = ?3",
                    params![target, updated_at, id],
                )
                .map_err(OfflineError::from)?;
                changed_notes.insert(id);
                *notes_list_changed = true;
            }
        }
        SyncPayload::NotePin {
            id,
            is_pinned,
            updated_at,
        } => {
            if lww_accept(tx, "note", &id, "is_pinned", updated_at, client_op_id)? {
                tx.execute(
                    "UPDATE notes SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3",
                    params![is_pinned, updated_at, id],
                )
                .map_err(OfflineError::from)?;
                changed_notes.insert(id);
                *notes_list_changed = true;
            }
        }
        SyncPayload::NoteDelete {
            id,
            deleted_at,
            updated_at,
        } => {
            if lww_accept(tx, "note", &id, "deleted_at", updated_at, client_op_id)? {
                tx.execute(
                    "UPDATE notes SET deleted_at = ?1, updated_at = ?2 WHERE id = ?3",
                    params![deleted_at, updated_at, id],
                )
                .map_err(OfflineError::from)?;
                reindex_tx(tx, &id)?;
                changed_notes.insert(id);
                *notes_list_changed = true;
            }
        }
    }
    Ok(())
}

fn lww_accept(
    tx: &Transaction<'_>,
    entity_type: &str,
    entity_id: &str,
    field: &str,
    updated_at: i64,
    client_op_id: &str,
) -> Result<bool, SyncError> {
    let cur: Option<(i64, String)> = tx
        .query_row(
            "SELECT updated_at, client_op_id FROM sync_lww WHERE entity_type = ?1 AND entity_id = ?2 AND field = ?3",
            params![entity_type, entity_id, field],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(OfflineError::from)?;
    if let Some((cur_ts, cur_op)) = cur
        && (updated_at, client_op_id) <= (cur_ts, cur_op.as_str())
    {
        return Ok(false);
    }
    tx.execute(
        "INSERT INTO sync_lww(entity_type, entity_id, field, updated_at, client_op_id)
         VALUES(?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(entity_type, entity_id, field) DO UPDATE SET
            updated_at = excluded.updated_at,
            client_op_id = excluded.client_op_id",
        params![entity_type, entity_id, field, updated_at, client_op_id],
    )
    .map_err(OfflineError::from)?;
    Ok(true)
}

fn folder_exists(tx: &Transaction<'_>, id: &str) -> Result<bool, OfflineError> {
    Ok(tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?1 AND deleted_at IS NULL)",
        [id],
        |row| row.get(0),
    )?)
}

fn bootstrap_outbox_tx(tx: &Transaction<'_>) -> Result<(), OfflineError> {
    let ts = now_ms();
    let folders: Vec<(String, String, String, i64, i64, i64)> = {
        let mut stmt = tx.prepare(
            "SELECT id, name, kind, sort_order, created_at, updated_at FROM folders WHERE kind = 'normal' AND deleted_at IS NULL",
        )?;
        stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    for (id, name, kind, sort_order, created_at, updated_at) in folders {
        insert_outbox_raw(
            tx,
            "folder",
            &id,
            &SyncPayload::FolderUpsert {
                id: id.clone(),
                name,
                kind,
                sort_order,
                created_at,
                updated_at,
            },
            &[],
            ts,
        )?;
    }

    struct BootstrapNoteRow {
        id: String,
        folder_id: String,
        crdt_state: Vec<u8>,
        is_pinned: bool,
        deleted_at: Option<i64>,
        created_at: i64,
        updated_at: i64,
    }
    let notes: Vec<BootstrapNoteRow> = {
        let mut stmt = tx.prepare(
            "SELECT id, folder_id, crdt_state, is_pinned, deleted_at, created_at, updated_at
             FROM notes WHERE is_locked = 0",
        )?;
        stmt.query_map([], |row| {
            Ok(BootstrapNoteRow {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                crdt_state: row.get(2)?,
                is_pinned: row.get(3)?,
                deleted_at: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?
    };
    for BootstrapNoteRow {
        id,
        folder_id,
        crdt_state,
        is_pinned,
        deleted_at,
        created_at,
        updated_at,
    } in notes {
        let atts = load_note_attachments(tx, &id)?;
        let hashes: Vec<String> = atts.iter().map(|a| a.plaintext_hash.clone()).collect();
        insert_outbox_raw(
            tx,
            "note",
            &id,
            &SyncPayload::NoteCreate {
                id: id.clone(),
                folder_id,
                crdt_state_b64: B64.encode(&crdt_state),
                created_at,
                updated_at,
            },
            &[],
            ts,
        )?;
        insert_outbox_raw(
            tx,
            "note",
            &id,
            &SyncPayload::NoteCrdtUpdate {
                id: id.clone(),
                crdt_state_b64: B64.encode(&crdt_state),
                updated_at,
                attachments: atts,
            },
            &hashes,
            ts,
        )?;
        if is_pinned {
            insert_outbox_raw(
                tx,
                "note",
                &id,
                &SyncPayload::NotePin {
                    id: id.clone(),
                    is_pinned: true,
                    updated_at,
                },
                &[],
                ts,
            )?;
        }
        if deleted_at.is_some() {
            insert_outbox_raw(
                tx,
                "note",
                &id,
                &SyncPayload::NoteDelete {
                    id: id.clone(),
                    deleted_at,
                    updated_at,
                },
                &[],
                ts,
            )?;
        }
    }
    Ok(())
}

fn sync_enabled(tx: &Transaction<'_>) -> Result<bool, OfflineError> {
    Ok(tx
        .query_row("SELECT enabled FROM sync_config WHERE id = 1", [], |row| {
            row.get::<_, i64>(0).map(|v| v != 0)
        })
        .optional()?
        .unwrap_or(false))
}

fn insert_outbox_raw(
    tx: &Transaction<'_>,
    entity_type: &str,
    entity_id: &str,
    payload: &SyncPayload,
    blob_hashes: &[String],
    ts: i64,
) -> Result<(), OfflineError> {
    let op_id = new_uuid_v7();
    let payload_json = serde_json::to_string(payload).map_err(|_| OfflineError::Invalid)?;
    let hashes_json = serde_json::to_string(blob_hashes).map_err(|_| OfflineError::Invalid)?;
    tx.execute(
        "INSERT INTO sync_outbox(client_op_id, entity_type, entity_id, payload_json, blob_hashes_json, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![op_id, entity_type, entity_id, payload_json, hashes_json, ts],
    )?;
    Ok(())
}

fn load_note_attachments(
    tx: &Transaction<'_>,
    note_id: &str,
) -> Result<Vec<SyncAttachmentMeta>, OfflineError> {
    let mut stmt = tx.prepare(
        "SELECT id, kind, file_name, mime_type, byte_size, content_hash, created_at
         FROM attachments WHERE note_id = ?1",
    )?;
    let rows = stmt
        .query_map([note_id], |row| {
            let hash: String = row.get(5)?;
            Ok(SyncAttachmentMeta {
                id: row.get(0)?,
                kind: row.get(1)?,
                file_name: row.get(2)?,
                mime_type: row.get(3)?,
                byte_size: row.get(4)?,
                plaintext_hash: hash.clone(),
                blob_id: hash,
                created_at: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub(crate) fn enqueue_folder_upsert(
    tx: &Transaction<'_>,
    id: &str,
    name: &str,
    kind: &str,
    sort_order: i64,
    created_at: i64,
    updated_at: i64,
) -> Result<(), OfflineError> {
    if !sync_enabled(tx)? {
        return Ok(());
    }
    insert_outbox_raw(
        tx,
        "folder",
        id,
        &SyncPayload::FolderUpsert {
            id: id.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            sort_order,
            created_at,
            updated_at,
        },
        &[],
        updated_at,
    )
}

pub(crate) fn enqueue_note_create(
    tx: &Transaction<'_>,
    id: &str,
    folder_id: &str,
    crdt_state: &[u8],
    created_at: i64,
    updated_at: i64,
) -> Result<(), OfflineError> {
    if !sync_enabled(tx)? {
        return Ok(());
    }
    insert_outbox_raw(
        tx,
        "note",
        id,
        &SyncPayload::NoteCreate {
            id: id.to_string(),
            folder_id: folder_id.to_string(),
            crdt_state_b64: B64.encode(crdt_state),
            created_at,
            updated_at,
        },
        &[],
        updated_at,
    )
}

pub(crate) fn enqueue_note_move(
    tx: &Transaction<'_>,
    id: &str,
    folder_id: &str,
    updated_at: i64,
) -> Result<(), OfflineError> {
    if !sync_enabled(tx)? {
        return Ok(());
    }
    insert_outbox_raw(
        tx,
        "note",
        id,
        &SyncPayload::NoteMove {
            id: id.to_string(),
            folder_id: folder_id.to_string(),
            updated_at,
        },
        &[],
        updated_at,
    )
}

pub(crate) fn enqueue_note_pin(
    tx: &Transaction<'_>,
    id: &str,
    is_pinned: bool,
    updated_at: i64,
) -> Result<(), OfflineError> {
    if !sync_enabled(tx)? {
        return Ok(());
    }
    insert_outbox_raw(
        tx,
        "note",
        id,
        &SyncPayload::NotePin {
            id: id.to_string(),
            is_pinned,
            updated_at,
        },
        &[],
        updated_at,
    )
}

pub(crate) fn enqueue_note_delete(
    tx: &Transaction<'_>,
    id: &str,
    deleted_at: Option<i64>,
    updated_at: i64,
) -> Result<(), OfflineError> {
    if !sync_enabled(tx)? {
        return Ok(());
    }
    insert_outbox_raw(
        tx,
        "note",
        id,
        &SyncPayload::NoteDelete {
            id: id.to_string(),
            deleted_at,
            updated_at,
        },
        &[],
        updated_at,
    )
}

pub(crate) fn enqueue_note_crdt(
    tx: &Transaction<'_>,
    id: &str,
    crdt_state: &[u8],
    updated_at: i64,
) -> Result<(), OfflineError> {
    if !sync_enabled(tx)? {
        return Ok(());
    }
    let atts = load_note_attachments(tx, id)?;
    let hashes: Vec<String> = atts.iter().map(|a| a.plaintext_hash.clone()).collect();
    insert_outbox_raw(
        tx,
        "note",
        id,
        &SyncPayload::NoteCrdtUpdate {
            id: id.to_string(),
            crdt_state_b64: B64.encode(crdt_state),
            updated_at,
            attachments: atts,
        },
        &hashes,
        updated_at,
    )
}

fn read_device_id(store: &Store) -> Result<Uuid, SyncError> {
    let raw: String = store
        .db()
        .query_row("SELECT device_id FROM sync_config WHERE id = 1", [], |r| r.get(0))
        .map_err(OfflineError::from)?;
    Uuid::parse_str(&raw).map_err(|e| SyncError::InvalidData(e.to_string()))
}

fn to_entity_uuid(id: &str) -> Uuid {
    Uuid::parse_str(id).unwrap_or_else(|_| Uuid::new_v5(&Uuid::NAMESPACE_OID, id.as_bytes()))
}

fn derive_recovery_auth_key(recovery_key: &SecretKey32) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"notes-e2ee-v1:recovery-auth:");
    hasher.update(recovery_key.as_bytes());
    hasher.finalize().into()
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
