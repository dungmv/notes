use notes_app::editor::{Batch, Operation, Style};
use notes_app::offline::Store;
use notes_app::sync::{SyncClient, SyncError};
use notes_sync::{
    AuthResponse, ChangeCredentialsPayload, DeviceInfoDto, DeviceListResponse, KeyBundleDto,
    LoginPayload, PresignDownloadResponse, PresignUploadResponse, PullSyncResponse, PulledDeltaDto,
    PushDeltaDto, PushReceiptDto, PushSyncResponse, RecoverBundleResponse, RecoverPayload,
    RegisterPayload, SaltResponse, SyncTransport, SyncTransportError,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tempfile::tempdir;
use uuid::Uuid;

#[derive(Default)]
struct ServerState {
    users: HashMap<String, MockUser>,
    devices: HashMap<Uuid, DeviceInfoDto>,
    deltas: Vec<PulledDeltaDto>,
    blobs: HashMap<String, Vec<u8>>,
    staging: HashMap<String, Vec<u8>>,
    tamper_next_pull: bool,
}

struct MockUser {
    id: Uuid,
    auth_key_b64: String,
    kdf_salt_b64: String,
    kdf_params: notes_crypto::KdfParams,
    wrapped_amk_b64: String,
    recovery_wrapped_amk_b64: Option<String>,
    recovery_auth_key_b64: Option<String>,
    key_version: u32,
}

struct MockTransport {
    state: Arc<Mutex<ServerState>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(ServerState::default())),
        }
    }

    fn require_active_device(s: &ServerState, device_id: Uuid) -> Result<(), SyncTransportError> {
        match s.devices.get(&device_id) {
            Some(d) if d.revoked_at.is_none() => Ok(()),
            _ => Err(SyncTransportError::Unauthorized),
        }
    }
}

impl SyncTransport for MockTransport {
    fn health(&self) -> Result<(), SyncTransportError> {
        Ok(())
    }

    fn get_salt(&self, email: &str) -> Result<SaltResponse, SyncTransportError> {
        let s = self.state.lock().unwrap();
        let u = s
            .users
            .get(email)
            .ok_or(SyncTransportError::Unauthorized)?;
        Ok(SaltResponse {
            kdf_salt_b64: u.kdf_salt_b64.clone(),
            kdf_params: u.kdf_params.clone(),
        })
    }

    fn register(&self, payload: &RegisterPayload) -> Result<AuthResponse, SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        let user_id = Uuid::now_v7();
        s.users.insert(
            payload.email.clone(),
            MockUser {
                id: user_id,
                auth_key_b64: payload.auth_key_b64.clone(),
                kdf_salt_b64: payload.kdf_salt_b64.clone(),
                kdf_params: payload.kdf_params.clone(),
                wrapped_amk_b64: payload.wrapped_amk_b64.clone(),
                recovery_wrapped_amk_b64: payload.recovery_wrapped_amk_b64.clone(),
                recovery_auth_key_b64: payload.recovery_auth_key_b64.clone(),
                key_version: 1,
            },
        );
        s.devices.insert(
            payload.device_id,
            DeviceInfoDto {
                id: payload.device_id,
                device_name: payload.device_name.clone(),
                platform: payload.platform.clone(),
                last_seen_at: 1,
                revoked_at: None,
                created_at: 1,
                is_current: true,
            },
        );
        Ok(AuthResponse {
            user_id,
            device_id: payload.device_id,
            access_token: format!("tok-{}", payload.device_id),
            refresh_token: format!("ref-{}", payload.device_id),
            key_bundle: KeyBundleDto {
                wrapped_amk_b64: payload.wrapped_amk_b64.clone(),
                recovery_wrapped_amk_b64: payload.recovery_wrapped_amk_b64.clone(),
                key_ring_encrypted_b64: None,
                key_version: 1,
            },
        })
    }

    fn login(&self, payload: &LoginPayload) -> Result<AuthResponse, SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        let u = s
            .users
            .get(&payload.email)
            .ok_or(SyncTransportError::Unauthorized)?;
        if u.auth_key_b64 != payload.auth_key_b64 {
            return Err(SyncTransportError::Unauthorized);
        }
        let user_id = u.id;
        let bundle = KeyBundleDto {
            wrapped_amk_b64: u.wrapped_amk_b64.clone(),
            recovery_wrapped_amk_b64: u.recovery_wrapped_amk_b64.clone(),
            key_ring_encrypted_b64: None,
            key_version: u.key_version,
        };
        s.devices.insert(
            payload.device_id,
            DeviceInfoDto {
                id: payload.device_id,
                device_name: payload.device_name.clone(),
                platform: payload.platform.clone(),
                last_seen_at: 3,
                revoked_at: None,
                created_at: 3,
                is_current: true,
            },
        );
        s.devices.insert(
            payload.device_id,
            DeviceInfoDto {
                id: payload.device_id,
                device_name: payload.device_name.clone(),
                platform: payload.platform.clone(),
                last_seen_at: 2,
                revoked_at: None,
                created_at: 2,
                is_current: true,
            },
        );
        Ok(AuthResponse {
            user_id,
            device_id: payload.device_id,
            access_token: format!("tok-{}", payload.device_id),
            refresh_token: format!("ref-{}", payload.device_id),
            key_bundle: bundle,
        })
    }

    fn refresh(&self, _refresh_token: &str, device_id: Uuid) -> Result<AuthResponse, SyncTransportError> {
        let s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        let u = s.users.values().next().unwrap();
        Ok(AuthResponse {
            user_id: u.id,
            device_id,
            access_token: format!("tok-{device_id}"),
            refresh_token: format!("ref2-{device_id}"),
            key_bundle: KeyBundleDto {
                wrapped_amk_b64: u.wrapped_amk_b64.clone(),
                recovery_wrapped_amk_b64: u.recovery_wrapped_amk_b64.clone(),
                key_ring_encrypted_b64: None,
                key_version: u.key_version,
            },
        })
    }

    fn change_credentials(
        &self,
        _access_token: &str,
        device_id: Uuid,
        payload: &ChangeCredentialsPayload,
    ) -> Result<(), SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        let u = s.users.values_mut().next().unwrap();
        if u.auth_key_b64 != payload.current_auth_key_b64 {
            return Err(SyncTransportError::Unauthorized);
        }
        u.auth_key_b64 = payload.new_auth_key_b64.clone();
        u.kdf_salt_b64 = payload.new_kdf_salt_b64.clone();
        u.kdf_params = payload.new_kdf_params.clone();
        u.wrapped_amk_b64 = payload.new_wrapped_amk_b64.clone();
        Ok(())
    }

    fn recover_bundle(
        &self,
        email: &str,
        recovery_auth_key_b64: &str,
    ) -> Result<RecoverBundleResponse, SyncTransportError> {
        let s = self.state.lock().unwrap();
        let u = s
            .users
            .get(email)
            .ok_or(SyncTransportError::Unauthorized)?;
        if u.recovery_auth_key_b64.as_deref() != Some(recovery_auth_key_b64) {
            return Err(SyncTransportError::Unauthorized);
        }
        Ok(RecoverBundleResponse {
            user_id: u.id,
            recovery_wrapped_amk_b64: u.recovery_wrapped_amk_b64.clone().unwrap(),
            key_version: u.key_version,
        })
    }

    fn recover(&self, payload: &RecoverPayload) -> Result<AuthResponse, SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        let u = s
            .users
            .get_mut(&payload.email)
            .ok_or(SyncTransportError::Unauthorized)?;
        if u.recovery_auth_key_b64.as_deref() != Some(&payload.recovery_auth_key_b64) {
            return Err(SyncTransportError::Unauthorized);
        }
        u.auth_key_b64 = payload.new_auth_key_b64.clone();
        u.kdf_salt_b64 = payload.new_kdf_salt_b64.clone();
        u.kdf_params = payload.new_kdf_params.clone();
        u.wrapped_amk_b64 = payload.new_wrapped_amk_b64.clone();
        let user_id = u.id;
        let bundle = KeyBundleDto {
            wrapped_amk_b64: u.wrapped_amk_b64.clone(),
            recovery_wrapped_amk_b64: u.recovery_wrapped_amk_b64.clone(),
            key_ring_encrypted_b64: None,
            key_version: u.key_version,
        };
        s.devices.insert(
            payload.device_id,
            DeviceInfoDto {
                id: payload.device_id,
                device_name: payload.device_name.clone(),
                platform: payload.platform.clone(),
                last_seen_at: 3,
                revoked_at: None,
                created_at: 3,
                is_current: true,
            },
        );
        Ok(AuthResponse {
            user_id,
            device_id: payload.device_id,
            access_token: format!("tok-{}", payload.device_id),
            refresh_token: format!("ref-{}", payload.device_id),
            key_bundle: bundle,
        })
    }

    fn list_devices(&self, _access_token: &str, device_id: Uuid) -> Result<DeviceListResponse, SyncTransportError> {
        let s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        Ok(DeviceListResponse {
            devices: s.devices.values().cloned().collect(),
        })
    }

    fn revoke_device(
        &self,
        _access_token: &str,
        device_id: Uuid,
        target_device_id: Uuid,
    ) -> Result<(), SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        if let Some(d) = s.devices.get_mut(&target_device_id) {
            d.revoked_at = Some(999);
        }
        Ok(())
    }

    fn push(
        &self,
        _access_token: &str,
        device_id: Uuid,
        deltas: &[PushDeltaDto],
    ) -> Result<PushSyncResponse, SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        let mut receipts = Vec::new();
        for d in deltas {
            if let Some(existing) = s.deltas.iter().find(|x| x.client_op_id == d.client_op_id) {
                receipts.push(PushReceiptDto {
                    client_op_id: d.client_op_id,
                    server_seq: existing.server_seq,
                });
            } else {
                let seq = (s.deltas.len() + 1) as i64;
                s.deltas.push(PulledDeltaDto {
                    server_seq: seq,
                    device_id,
                    entity_type: d.entity_type.clone(),
                    entity_id: d.entity_id,
                    client_op_id: d.client_op_id,
                    crypto_version: d.crypto_version,
                    key_version: d.key_version,
                    encrypted_payload_b64: d.encrypted_payload_b64.clone(),
                    created_at: 1000,
                });
                receipts.push(PushReceiptDto {
                    client_op_id: d.client_op_id,
                    server_seq: seq,
                });
            }
        }
        Ok(PushSyncResponse { receipts })
    }

    fn pull(
        &self,
        _access_token: &str,
        device_id: Uuid,
        since_seq: i64,
        _limit: u32,
    ) -> Result<PullSyncResponse, SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        let mut items: Vec<PulledDeltaDto> = s
            .deltas
            .iter()
            .filter(|d| d.server_seq > since_seq)
            .take(2)
            .cloned()
            .collect();
        if s.tamper_next_pull && !items.is_empty() {
            s.tamper_next_pull = false;
            items[0].entity_id = Uuid::now_v7();
        }
        let next_cursor = items.last().map(|d| d.server_seq).unwrap_or(since_seq);
        let has_more = s.deltas.iter().any(|d| d.server_seq > next_cursor);
        Ok(PullSyncResponse {
            deltas: items,
            next_cursor,
            has_more,
        })
    }

    fn presign_upload(
        &self,
        _access_token: &str,
        device_id: Uuid,
        content_hash: &str,
        _byte_size: u64,
        _ciphertext_sha256: &str,
    ) -> Result<PresignUploadResponse, SyncTransportError> {
        let s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        if s.blobs.contains_key(content_hash) {
            return Ok(PresignUploadResponse {
                exists: true,
                upload_url: None,
                upload_token: None,
            });
        }
        Ok(PresignUploadResponse {
            exists: false,
            upload_url: Some(format!("mock://upload/{content_hash}")),
            upload_token: Some(content_hash.to_string()),
        })
    }

    fn upload_presigned(&self, upload_url: &str, bytes: &[u8]) -> Result<(), SyncTransportError> {
        let key = upload_url.trim_start_matches("mock://upload/").to_string();
        self.state.lock().unwrap().staging.insert(key, bytes.to_vec());
        Ok(())
    }

    fn complete_upload(
        &self,
        _access_token: &str,
        device_id: Uuid,
        upload_token: &str,
    ) -> Result<(), SyncTransportError> {
        let mut s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        let bytes = s.staging.remove(upload_token).unwrap();
        s.blobs.insert(upload_token.to_string(), bytes);
        Ok(())
    }

    fn presign_download(
        &self,
        _access_token: &str,
        device_id: Uuid,
        content_hash: &str,
    ) -> Result<PresignDownloadResponse, SyncTransportError> {
        let s = self.state.lock().unwrap();
        Self::require_active_device(&s, device_id)?;
        if !s.blobs.contains_key(content_hash) {
            return Err(SyncTransportError::Http {
                status: 404,
                message: "not found".into(),
            });
        }
        Ok(PresignDownloadResponse {
            download_url: format!("mock://download/{content_hash}"),
        })
    }

    fn download_presigned(&self, download_url: &str) -> Result<Vec<u8>, SyncTransportError> {
        let key = download_url.trim_start_matches("mock://download/");
        Ok(self.state.lock().unwrap().blobs.get(key).unwrap().clone())
    }
}

const TINY_PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15c4";

#[test]
fn two_devices_converge_with_offline_edits_blobs_and_recovery() {
    let transport = Arc::new(MockTransport::new());
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();

    let mut store_a = Store::open(dir_a.path()).unwrap();
    let mut store_b = Store::open(dir_b.path()).unwrap();

    // Device A creates offline note & folder before enabling sync
    let folder_id = store_a.create_folder("Dự án M3", None).unwrap();
    let note_id = store_a.create_note(&folder_id).unwrap();
    let mut ed_a = store_a.open_note(&note_id).unwrap();
    let blk_id = ed_a.snapshot().blocks[0].id.clone();
    store_a
        .apply(
            &mut ed_a,
            Batch {
                revision: 0,
                operations: vec![Operation::Replace {
                    id: blk_id.clone(),
                    cell: None,
                    start: 0,
                    end: 0,
                    text: "Ghi chú ban đầu".to_string(),
                }],
                external: false,
            },
        )
        .unwrap();

    // Register on A -> bootstraps offline note into outbox
    let mut sync_a = SyncClient::with_transport(transport.clone());
    let reg = sync_a
        .register(
            &mut store_a,
            "https://mock.local",
            "user@example.com",
            "pass-123",
            "Win11-A",
            "windows",
        )
        .unwrap();
    let recovery_key = reg.recovery_key.unwrap();
    let res_a = sync_a.sync_now(&mut store_a).unwrap();
    assert!(res_a.pushed >= 3);

    // Login on B and pull
    let mut sync_b = SyncClient::with_transport(transport.clone());
    sync_b
        .login(
            &mut store_b,
            "https://mock.local",
            "user@example.com",
            "pass-123",
            "macOS-B",
            "macos",
        )
        .unwrap();
    let res_b = sync_b.sync_now(&mut store_b).unwrap();
    assert!(res_b.pulled >= 3);
    assert_eq!(
        serde_json::to_value(store_b.note(&note_id).unwrap()).unwrap()["title"],
        "Ghi chú ban đầu"
    );

    // Both A and B edit offline concurrently (independent edits + image on A)
    let mut ed_a = store_a.open_note(&note_id).unwrap();
    let rev_a = ed_a.snapshot().revision;
    let upd_a = store_a
        .apply(
            &mut ed_a,
            Batch {
                revision: rev_a,
                operations: vec![Operation::SplitParagraph {
                    id: blk_id.clone(),
                    offset: "Ghi chú ban đầu".encode_utf16().count() as u32,
                    new_id: "blk-a".to_string(),
                }],
                external: false,
            },
        )
        .unwrap();
    let upd_a = store_a
        .apply(
            &mut ed_a,
            Batch {
                revision: upd_a.revision,
                operations: vec![Operation::Replace {
                    id: "blk-a".to_string(),
                    cell: None,
                    start: 0,
                    end: 0,
                    text: "Phần từ Windows".to_string(),
                }],
                external: false,
            },
        )
        .unwrap();
    store_a
        .paste_image(&mut ed_a, upd_a.revision, "blk-a".to_string(), 0, 0, TINY_PNG)
        .unwrap();

    let mut ed_b = store_b.open_note(&note_id).unwrap();
    let rev_b = ed_b.snapshot().revision;
    store_b
        .apply(
            &mut ed_b,
            Batch {
                revision: rev_b,
                operations: vec![Operation::Format {
                    id: blk_id.clone(),
                    cell: None,
                    start: 0,
                    end: 7,
                    style: Style::Bold,
                }],
                external: false,
            },
        )
        .unwrap();

    // Sync A -> Sync B -> Sync A
    sync_a.sync_now(&mut store_a).unwrap();
    sync_b.sync_now(&mut store_b).unwrap();
    sync_a.sync_now(&mut store_a).unwrap();

    let snap_a = store_a.open_note(&note_id).unwrap().snapshot().clone();
    let snap_b = store_b.open_note(&note_id).unwrap().snapshot().clone();
    assert_eq!(snap_a.blocks, snap_b.blocks);

    // Tampered pull fails without advancing cursor
    let cursor_before = sync_b.status(&store_b).unwrap().last_server_cursor;
    let mut ed_a = store_a.open_note(&note_id).unwrap();
    let rev_a = ed_a.snapshot().revision;
    store_a
        .apply(
            &mut ed_a,
            Batch {
                revision: rev_a,
                operations: vec![Operation::Replace {
                    id: blk_id.clone(),
                    cell: None,
                    start: 0,
                    end: 0,
                    text: "X ".to_string(),
                }],
                external: false,
            },
        )
        .unwrap();
    sync_a.sync_now(&mut store_a).unwrap();
    transport.state.lock().unwrap().tamper_next_pull = true;
    assert!(sync_b.sync_now(&mut store_b).is_err());
    assert_eq!(
        sync_b.status(&store_b).unwrap().last_server_cursor,
        cursor_before
    );
    // Clean retry succeeds
    sync_b.sync_now(&mut store_b).unwrap();

    // Password change & Recovery Key flow
    sync_a
        .change_password(&store_a, "pass-123", "pass-456")
        .unwrap();
    let mut sync_c = SyncClient::with_transport(transport.clone());
    let dir_c = tempdir().unwrap();
    let mut store_c = Store::open(dir_c.path()).unwrap();
    sync_c
        .recover_account(
            &mut store_c,
            "https://mock.local",
            "user@example.com",
            &recovery_key,
            "pass-789",
            "Win11-C",
            "windows",
        )
        .unwrap();
    sync_c.sync_now(&mut store_c).unwrap();
    assert_eq!(
        store_c.open_note(&note_id).unwrap().snapshot().blocks,
        store_a.open_note(&note_id).unwrap().snapshot().blocks
    );

    // Revoke device B
    let dev_b_id = uuid::Uuid::parse_str(&sync_b.status(&store_b).unwrap().device_id).unwrap();
    sync_a.revoke_device(&store_a, dev_b_id).unwrap();
    assert!(matches!(
        sync_b.sync_now(&mut store_b),
        Err(SyncError::Transport(notes_sync::SyncTransportError::Unauthorized))
    ));
}

#[test]
#[ignore = "requires live backend on NOTES_LIVE_SERVER"]
fn live_backend_two_device_e2ee_sync_with_blobs_and_recovery() {
    let base_url = std::env::var("NOTES_LIVE_SERVER")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    let email = format!("live-{}@example.com", Uuid::now_v7());

    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    let dir_c = tempdir().unwrap();

    let mut store_a = Store::open(dir_a.path()).unwrap();
    let mut store_b = Store::open(dir_b.path()).unwrap();
    let mut store_c = Store::open(dir_c.path()).unwrap();

    let folder_id = store_a.create_folder("Live Sync M3", None).unwrap();
    let note_id = store_a.create_note(&folder_id).unwrap();
    let mut ed_a = store_a.open_note(&note_id).unwrap();
    let blk_id = ed_a.snapshot().blocks[0].id.clone();
    let u1 = store_a
        .apply(
            &mut ed_a,
            Batch {
                revision: 0,
                operations: vec![Operation::Replace {
                    id: blk_id.clone(),
                    cell: None,
                    start: 0,
                    end: 0,
                    text: "Nội dung E2EE thật".to_string(),
                }],
                external: false,
            },
        )
        .unwrap();
    store_a
        .paste_image(&mut ed_a, u1.revision, blk_id.clone(), 0, 0, TINY_PNG)
        .unwrap();

    let mut sync_a = SyncClient::new();
    let reg = sync_a
        .register(
            &mut store_a,
            &base_url,
            &email,
            "live-pass-1",
            "Win11-Live-A",
            "windows",
        )
        .unwrap();
    let recovery_key = reg.recovery_key.unwrap();
    let res_a = sync_a.sync_now(&mut store_a).unwrap();
    assert!(res_a.pushed >= 3);

    let mut sync_b = SyncClient::new();
    sync_b
        .login(
            &mut store_b,
            &base_url,
            &email,
            "live-pass-1",
            "Win11-Live-B",
            "windows",
        )
        .unwrap();
    let res_b = sync_b.sync_now(&mut store_b).unwrap();
    assert!(res_b.pulled >= 3);
    assert_eq!(
        store_b.open_note(&note_id).unwrap().snapshot().blocks,
        store_a.open_note(&note_id).unwrap().snapshot().blocks
    );

    // Concurrent edits on A & B
    let mut ed_a = store_a.open_note(&note_id).unwrap();
    let rev_a = ed_a.snapshot().revision;
    store_a
        .apply(
            &mut ed_a,
            Batch {
                revision: rev_a,
                operations: vec![Operation::Replace {
                    id: blk_id.clone(),
                    cell: None,
                    start: 0,
                    end: 0,
                    text: "[A] ".to_string(),
                }],
                external: false,
            },
        )
        .unwrap();

    let mut ed_b = store_b.open_note(&note_id).unwrap();
    let rev_b = ed_b.snapshot().revision;
    let text_blk_id = ed_b
        .snapshot()
        .blocks
        .iter()
        .find_map(|b| match &b.kind {
            notes_app::editor::BlockKind::Paragraph { content } if !content.text.is_empty() => {
                Some(b.id.clone())
            }
            _ => None,
        })
        .unwrap();
    store_b
        .apply(
            &mut ed_b,
            Batch {
                revision: rev_b,
                operations: vec![Operation::Format {
                    id: text_blk_id,
                    cell: None,
                    start: 0,
                    end: 3,
                    style: Style::Bold,
                }],
                external: false,
            },
        )
        .unwrap();

    sync_a.sync_now(&mut store_a).unwrap();
    sync_b.sync_now(&mut store_b).unwrap();
    sync_a.sync_now(&mut store_a).unwrap();
    assert_eq!(
        store_a.open_note(&note_id).unwrap().snapshot().blocks,
        store_b.open_note(&note_id).unwrap().snapshot().blocks
    );

    // Password change + Recovery + Revoke B on real Spring Boot backend
    sync_a
        .change_password(&store_a, "live-pass-1", "live-pass-2")
        .unwrap();
    let mut sync_c = SyncClient::new();
    sync_c
        .recover_account(
            &mut store_c,
            &base_url,
            &email,
            &recovery_key,
            "live-pass-3",
            "Win11-Live-C",
            "windows",
        )
        .unwrap();
    sync_c.sync_now(&mut store_c).unwrap();
    assert_eq!(
        store_c.open_note(&note_id).unwrap().snapshot().blocks,
        store_a.open_note(&note_id).unwrap().snapshot().blocks
    );

    let dev_b_id = uuid::Uuid::parse_str(&sync_b.status(&store_b).unwrap().device_id).unwrap();
    sync_a.revoke_device(&store_a, dev_b_id).unwrap();
    assert!(matches!(
        sync_b.sync_now(&mut store_b),
        Err(SyncError::Transport(notes_sync::SyncTransportError::Unauthorized))
    ));
}
