use notes_crypto::KdfParams;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SyncTransportError {
    #[error("Mất kết nối mạng hoặc máy chủ không phản hồi: {0}")]
    Network(String),
    #[error("Phiên đăng nhập hết hạn hoặc thiết bị đã bị thu hồi")]
    Unauthorized,
    #[error("Xung đột dữ liệu đồng bộ: {0}")]
    Conflict(String),
    #[error("Lỗi máy chủ ({status}): {message}")]
    Http { status: u16, message: String },
    #[error("Dữ liệu phản hồi không hợp lệ: {0}")]
    InvalidResponse(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SaltResponse {
    pub kdf_salt_b64: String,
    pub kdf_params: KdfParams,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyBundleDto {
    pub wrapped_amk_b64: String,
    pub recovery_wrapped_amk_b64: Option<String>,
    pub key_ring_encrypted_b64: Option<String>,
    pub key_version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub key_bundle: KeyBundleDto,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoverBundleResponse {
    pub user_id: Uuid,
    pub recovery_wrapped_amk_b64: String,
    pub key_version: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceInfoDto {
    pub id: Uuid,
    pub device_name: String,
    pub platform: String,
    pub last_seen_at: i64,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
    pub is_current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfoDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushDeltaDto {
    pub client_op_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub crypto_version: u8,
    pub key_version: u32,
    pub encrypted_payload_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushReceiptDto {
    pub client_op_id: Uuid,
    pub server_seq: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushSyncResponse {
    pub receipts: Vec<PushReceiptDto>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PulledDeltaDto {
    pub server_seq: i64,
    pub device_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub client_op_id: Uuid,
    pub crypto_version: u8,
    pub key_version: u32,
    pub encrypted_payload_b64: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullSyncResponse {
    pub deltas: Vec<PulledDeltaDto>,
    pub next_cursor: i64,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresignUploadResponse {
    pub exists: bool,
    pub upload_url: Option<String>,
    pub upload_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresignDownloadResponse {
    pub download_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterPayload {
    pub email: String,
    pub auth_key_b64: String,
    pub kdf_salt_b64: String,
    pub kdf_params: KdfParams,
    pub wrapped_amk_b64: String,
    pub recovery_wrapped_amk_b64: Option<String>,
    pub recovery_auth_key_b64: Option<String>,
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginPayload {
    pub email: String,
    pub auth_key_b64: String,
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeCredentialsPayload {
    pub current_auth_key_b64: String,
    pub new_auth_key_b64: String,
    pub new_kdf_salt_b64: String,
    pub new_kdf_params: KdfParams,
    pub new_wrapped_amk_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoverPayload {
    pub email: String,
    pub recovery_auth_key_b64: String,
    pub new_auth_key_b64: String,
    pub new_kdf_salt_b64: String,
    pub new_kdf_params: KdfParams,
    pub new_wrapped_amk_b64: String,
    pub device_id: Uuid,
    pub device_name: String,
    pub platform: String,
}

pub trait SyncTransport: Send + Sync {
    fn health(&self) -> Result<(), SyncTransportError>;
    fn get_salt(&self, email: &str) -> Result<SaltResponse, SyncTransportError>;
    fn register(&self, payload: &RegisterPayload) -> Result<AuthResponse, SyncTransportError>;
    fn login(&self, payload: &LoginPayload) -> Result<AuthResponse, SyncTransportError>;
    fn refresh(&self, refresh_token: &str, device_id: Uuid) -> Result<AuthResponse, SyncTransportError>;
    fn change_credentials(
        &self,
        access_token: &str,
        device_id: Uuid,
        payload: &ChangeCredentialsPayload,
    ) -> Result<(), SyncTransportError>;
    fn recover_bundle(
        &self,
        email: &str,
        recovery_auth_key_b64: &str,
    ) -> Result<RecoverBundleResponse, SyncTransportError>;
    fn recover(&self, payload: &RecoverPayload) -> Result<AuthResponse, SyncTransportError>;
    fn list_devices(&self, access_token: &str, device_id: Uuid) -> Result<DeviceListResponse, SyncTransportError>;
    fn revoke_device(
        &self,
        access_token: &str,
        device_id: Uuid,
        target_device_id: Uuid,
    ) -> Result<(), SyncTransportError>;
    fn push(
        &self,
        access_token: &str,
        device_id: Uuid,
        deltas: &[PushDeltaDto],
    ) -> Result<PushSyncResponse, SyncTransportError>;
    fn pull(
        &self,
        access_token: &str,
        device_id: Uuid,
        since_seq: i64,
        limit: u32,
    ) -> Result<PullSyncResponse, SyncTransportError>;
    fn presign_upload(
        &self,
        access_token: &str,
        device_id: Uuid,
        content_hash: &str,
        byte_size: u64,
        ciphertext_sha256: &str,
    ) -> Result<PresignUploadResponse, SyncTransportError>;
    fn upload_presigned(&self, upload_url: &str, bytes: &[u8]) -> Result<(), SyncTransportError>;
    fn complete_upload(
        &self,
        access_token: &str,
        device_id: Uuid,
        upload_token: &str,
    ) -> Result<(), SyncTransportError>;
    fn presign_download(
        &self,
        access_token: &str,
        device_id: Uuid,
        content_hash: &str,
    ) -> Result<PresignDownloadResponse, SyncTransportError>;
    fn download_presigned(&self, download_url: &str) -> Result<Vec<u8>, SyncTransportError>;
}

pub struct HttpSyncTransport {
    base_url: String,
    agent: ureq::Agent,
}

impl HttpSyncTransport {
    pub fn new(base_url: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn map_err(err: ureq::Error) -> SyncTransportError {
        match err {
            ureq::Error::Status(401 | 403, _) => SyncTransportError::Unauthorized,
            ureq::Error::Status(409, resp) => {
                let msg = resp.into_string().unwrap_or_default();
                SyncTransportError::Conflict(msg)
            }
            ureq::Error::Status(status, resp) => {
                let msg = resp.into_string().unwrap_or_default();
                SyncTransportError::Http { status, message: msg }
            }
            ureq::Error::Transport(t) => SyncTransportError::Network(t.to_string()),
        }
    }

    fn read_json<T: for<'de> Deserialize<'de>>(resp: ureq::Response) -> Result<T, SyncTransportError> {
        resp.into_json::<T>()
            .map_err(|e| SyncTransportError::InvalidResponse(e.to_string()))
    }
}

impl SyncTransport for HttpSyncTransport {
    fn health(&self) -> Result<(), SyncTransportError> {
        self.agent
            .get(&self.url("/actuator/health"))
            .call()
            .map_err(Self::map_err)?;
        Ok(())
    }

    fn get_salt(&self, email: &str) -> Result<SaltResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/auth/salt"))
            .send_json(serde_json::json!({ "email": email }))
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn register(&self, payload: &RegisterPayload) -> Result<AuthResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/auth/register"))
            .send_json(serde_json::to_value(payload).unwrap())
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn login(&self, payload: &LoginPayload) -> Result<AuthResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/auth/login"))
            .send_json(serde_json::to_value(payload).unwrap())
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn refresh(&self, refresh_token: &str, device_id: Uuid) -> Result<AuthResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/auth/refresh"))
            .send_json(serde_json::json!({
                "refresh_token": refresh_token,
                "device_id": device_id,
            }))
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn change_credentials(
        &self,
        access_token: &str,
        device_id: Uuid,
        payload: &ChangeCredentialsPayload,
    ) -> Result<(), SyncTransportError> {
        self.agent
            .put(&self.url("/v1/auth/credentials"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .send_json(serde_json::to_value(payload).unwrap())
            .map_err(Self::map_err)?;
        Ok(())
    }

    fn recover_bundle(
        &self,
        email: &str,
        recovery_auth_key_b64: &str,
    ) -> Result<RecoverBundleResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/auth/recover/bundle"))
            .send_json(serde_json::json!({
                "email": email,
                "recovery_auth_key_b64": recovery_auth_key_b64,
            }))
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn recover(&self, payload: &RecoverPayload) -> Result<AuthResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/auth/recover"))
            .send_json(serde_json::to_value(payload).unwrap())
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn list_devices(&self, access_token: &str, device_id: Uuid) -> Result<DeviceListResponse, SyncTransportError> {
        let resp = self
            .agent
            .get(&self.url("/v1/devices"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .call()
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn revoke_device(
        &self,
        access_token: &str,
        device_id: Uuid,
        target_device_id: Uuid,
    ) -> Result<(), SyncTransportError> {
        self.agent
            .delete(&self.url(&format!("/v1/devices/{target_device_id}")))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .call()
            .map_err(Self::map_err)?;
        Ok(())
    }

    fn push(
        &self,
        access_token: &str,
        device_id: Uuid,
        deltas: &[PushDeltaDto],
    ) -> Result<PushSyncResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/sync/push"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .send_json(serde_json::json!({ "deltas": deltas }))
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn pull(
        &self,
        access_token: &str,
        device_id: Uuid,
        since_seq: i64,
        limit: u32,
    ) -> Result<PullSyncResponse, SyncTransportError> {
        let resp = self
            .agent
            .get(&self.url("/v1/sync/pull"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .query("since_seq", &since_seq.to_string())
            .query("limit", &limit.to_string())
            .call()
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn presign_upload(
        &self,
        access_token: &str,
        device_id: Uuid,
        content_hash: &str,
        byte_size: u64,
        ciphertext_sha256: &str,
    ) -> Result<PresignUploadResponse, SyncTransportError> {
        let resp = self
            .agent
            .post(&self.url("/v1/blobs/presign-upload"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .send_json(serde_json::json!({
                "content_hash": content_hash,
                "byte_size": byte_size,
                "ciphertext_sha256": ciphertext_sha256,
            }))
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn upload_presigned(&self, upload_url: &str, bytes: &[u8]) -> Result<(), SyncTransportError> {
        self.agent
            .put(upload_url)
            .send_bytes(bytes)
            .map_err(Self::map_err)?;
        Ok(())
    }

    fn complete_upload(
        &self,
        access_token: &str,
        device_id: Uuid,
        upload_token: &str,
    ) -> Result<(), SyncTransportError> {
        self.agent
            .post(&self.url("/v1/blobs/complete-upload"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .send_json(serde_json::json!({ "upload_token": upload_token }))
            .map_err(Self::map_err)?;
        Ok(())
    }

    fn presign_download(
        &self,
        access_token: &str,
        device_id: Uuid,
        content_hash: &str,
    ) -> Result<PresignDownloadResponse, SyncTransportError> {
        let resp = self
            .agent
            .get(&self.url("/v1/blobs/presign-download"))
            .set("Authorization", &format!("Bearer {access_token}"))
            .set("X-Device-Id", &device_id.to_string())
            .query("hash", content_hash)
            .call()
            .map_err(Self::map_err)?;
        Self::read_json(resp)
    }

    fn download_presigned(&self, download_url: &str) -> Result<Vec<u8>, SyncTransportError> {
        let resp = self.agent.get(download_url).call().map_err(Self::map_err)?;
        let mut buf = Vec::new();
        resp.into_reader()
            .read_to_end(&mut buf)
            .map_err(|e| SyncTransportError::Network(e.to_string()))?;
        Ok(buf)
    }
}
