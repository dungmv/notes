# 06. Backend Service (Java / Spring Boot)

Tài liệu này đặc tả kiến trúc backend cho tính năng đồng bộ đa thiết bị sử dụng **Java 21+ / Spring Boot 4.x**, hướng tới tính độc lập hạ tầng (**không vendor lock-in**): có thể tự host (self-hosted) bằng Docker Compose hoặc triển khai lên bất kỳ cloud/VPS nào.

---

## 1. Kiến trúc Tổng thể & Tech Stack

```text
+-------------------+         HTTPS           +---------------------------------------+
|  macOS / Windows  | ──────────────────────► |       Spring Boot 4 (Java 21+)        |
|  Linux / Mobile   |                         |  Stateless REST API (Virtual Threads) |
|   (notes-sync)    | ◄────────────────────── |  Spring Web + Spring Security (JWT)   |
+----+---------+----+   Presigned URL (S3)    +--------+---------------------+--------+
     │         │                                       │                     │
     │         │ Direct Blob I/O                       │ JDBC / Flyway       │ HTTP / gRPC Publish
     │         ▼                                       ▼                     ▼
     │  +-------------------+                 +----------------+    +-----------------+
     │  | S3-Compatible     |                 |  PostgreSQL 16 |    |   Centrifugo    |
     │  | (MinIO / S3 / B2) |                 | (Sync Deltas & |    | (Real-time Push |
     │  | (Encrypted Blobs) |                 |   Metadata)    |    |  & Connections) |
     │  +-------------------+                 +----------------+    +--------+--------+
     │                                                                       ▲
     └────────────────────────── Subscribe (sync:user#<id>) ─────────────────┘
```

### Thành phần Công nghệ
- **Runtime & Framework**: **Java 21+** (bật **Virtual Threads** `spring.threads.virtual.enabled=true`) + **Spring Boot 4.x** (Spring Web, Spring Security, Spring Data JDBC / jOOQ). Spring Boot hoàn toàn **stateless**, không giữ kết nối dài hạn.
- **Database**: **PostgreSQL 16+** kết hợp **Flyway** cho schema migrations.
- **Object Storage (Chuẩn S3)**: Giao tiếp qua chuẩn S3 API (`software.amazon.awssdk:s3` + `S3Presigner`), tương thích **MinIO** (self-host), AWS S3, Backblaze B2, hoặc Wasabi. Client upload/download file đính kèm trực tiếp bằng Presigned URL để giảm tải cho Spring Boot server.
- **Real-time Push**: **Centrifugo** (self-hosted real-time messaging server). Tách biệt hoàn toàn việc giữ hàng chục nghìn kết nối client ra khỏi Spring Boot; Spring Boot chỉ cấp Connection JWT cho client và gọi API `publish` của Centrifugo khi có thay đổi mới.
- **Đóng gói**: Docker image (hỗ trợ cả **JVM** và **GraalVM Native Image** nếu muốn tối ưu RAM/startup) + `docker-compose.yml` chuẩn.

---

## 2. Cấu trúc Project Backend

```text
backend/
├── pom.xml (hoặc build.gradle.kts)
├── Dockerfile
├── docker-compose.yml                  # Spring Boot + PostgreSQL + MinIO + Centrifugo
└── src/main/
    ├── resources/
    │   ├── application.yml
    │   └── db/migration/
    │       └── V1__init_schema.sql     # Flyway migration
    └── java/com/notes/backend/
        ├── common/                     # SecurityConfig, S3Config, CentrifugoConfig, GlobalExceptionHandler
        └── features/                   # Tổ chức theo feature (giống NestJS)
            ├── auth/                   # AuthController, AuthService, UserRepository, dto/, model/
            ├── sync/                   # SyncController, SyncService, SyncRepository, dto/, model/
            ├── blob/                   # BlobController, BlobService, BlobRepository, dto/
            └── realtime/               # CentrifugoPublisher, CentrifugoTokenService
```

---

## 3. PostgreSQL Schema

Do mô hình **Zero-Knowledge E2EE** (xem chi tiết mật mã tại [07-e2ee-cryptography.md](./07-e2ee-cryptography.md)), PostgreSQL **không lưu plaintext** của tiêu đề, nội dung ghi chú hay tên thư mục. Mọi payload đều được mã hóa từ `notes-crypto` ở phía client trước khi gửi lên API.

```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email_hash TEXT UNIQUE NOT NULL,
    auth_verifier TEXT NOT NULL,       -- Server-side Argon2id hash của Auth Key (AK)
    kdf_salt BYTEA NOT NULL,           -- 32-byte salt cho client Argon2id
    kdf_params_json JSONB NOT NULL,    -- Tham số Argon2id (m_kib, t, p)
    wrapped_amk BYTEA NOT NULL,        -- Account Master Key đã wrap bởi KEK
    recovery_wrapped_amk BYTEA,        -- Bản wrap dự phòng bởi 256-bit Recovery Key
    key_ring_encrypted BYTEA,          -- Lịch sử AMK cũ (khi xoay khóa)
    key_version INT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL
);

CREATE TABLE devices (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_name TEXT NOT NULL,
    platform TEXT NOT NULL,            -- 'macos' | 'windows' | 'linux' | 'ios' | 'android'
    last_seen_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

-- Bảng nhật ký đồng bộ (Append-only Sync Oplog)
CREATE TABLE sync_deltas (
    server_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL,
    entity_type TEXT NOT NULL,         -- 'folder' | 'note' | 'attachment_meta'
    entity_id UUID NOT NULL,
    client_op_id UUID NOT NULL,        -- Idempotency key tránh trùng lặp khi retry
    crypto_version SMALLINT NOT NULL DEFAULT 1,
    key_version INT NOT NULL DEFAULT 1,
    encrypted_payload BYTEA NOT NULL,  -- Envelope ciphertext (CRDT delta hoặc folder metadata)
    created_at BIGINT NOT NULL,
    UNIQUE (user_id, client_op_id)
);

CREATE INDEX idx_sync_deltas_user_seq ON sync_deltas(user_id, server_seq);
CREATE INDEX idx_sync_deltas_entity ON sync_deltas(user_id, entity_id, server_seq);

-- Bảng theo dõi các blob đính kèm trên S3/MinIO (deduplication & dọn rác)
CREATE TABLE user_blobs (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,        -- SHA-256 của ciphertext blob
    byte_size BIGINT NOT NULL,
    storage_key TEXT NOT NULL,
    ref_count INT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, content_hash)
);
```

---

## 4. API Endpoints & Giao thức Đồng bộ

Tất cả endpoint (trừ `/v1/auth/**`) được bảo vệ bởi **Spring Security** với `Authorization: Bearer <jwt>` và header `X-Device-Id: <device_uuid>`.

### 4.1. Push Deltas (`POST /v1/sync/push`)
- **Request**:
  ```json
  {
    "deltas": [
      {
        "client_op_id": "uuid-v7",
        "entity_type": "note",
        "entity_id": "note-uuid",
        "encrypted_payload_b64": "..."
      }
    ]
  }
  ```
- **Xử lý trên Spring Boot**:
  1. Trong `@Transactional`, dùng `JdbcTemplate.batchUpdate` với `INSERT INTO sync_deltas (...) VALUES (...) ON CONFLICT (user_id, client_op_id) DO NOTHING` để đảm bảo **idempotent** và hiệu năng cao.
  2. Lấy `MAX(server_seq)` vừa tạo, gọi `CentrifugoPublisher` (qua Centrifugo HTTP/gRPC `publish` API) đẩy thông báo tới channel `sync:user#<user_id>` kèm `latest_seq` và `source_device_id` để các thiết bị khác tự động kéo delta mới.
- **Response**: `{ "ack_seq": 1042, "accepted_count": 3 }`.

### 4.2. Pull Deltas (`GET /v1/sync/pull?since_seq=1000&limit=200`)
- **Xử lý trên Spring Boot**:
  - Truy vấn `sync_deltas` theo `user_id` và `server_seq > since_seq` với `ORDER BY server_seq ASC LIMIT :limit`.
- **Response**:
  ```json
  {
    "deltas": [ ... ],
    "latest_seq": 1042,
    "has_more": false
  }
  ```
- Client lặp lại `pull` cho tới khi `has_more == false`, sau đó giải mã và nạp từng delta vào `notes-crdt`.

### 4.3. Compaction (`POST /v1/sync/compact`)
- Khi một ghi chú tích lũy quá nhiều delta nhỏ (ví dụ > 200 deltas), client gửi bản snapshot CRDT đã gộp kèm `up_to_seq`.
- Server trong một `@Transactional` chèn snapshot mới và xóa các `sync_deltas` của `entity_id` có `server_seq <= up_to_seq`.

### 4.4. Blob Upload / Download (`S3Presigner`)
- **`POST /v1/blobs/presign-upload`**:
  - Client gửi `{ content_hash, byte_size }`.
  - Server kiểm tra `user_blobs`: nếu đã tồn tại, trả về `exists: true` (deduplication, không cần upload lại).
  - Nếu chưa có, `S3Presigner` tạo Presigned PUT URL trỏ tới `<user_id>/<content_hash>` (hết hạn sau 5 phút).
- **`GET /v1/blobs/presign-download?hash=<content_hash>`**:
  - Kiểm tra quyền sở hữu trong `user_blobs`, trả về Presigned GET URL.

### 4.5. Real-time Notification (Centrifugo)
- Spring Boot cung cấp endpoint `GET /v1/auth/realtime-token` trả về **Centrifugo Connection JWT** (chứa `sub = user_id`, `exp`) để client kết nối trực tiếp tới Centrifugo.
- Client (`notes-sync`) subscribe vào private/user-limited channel `sync:user#<user_id>`.
- Centrifugo chỉ truyền tín hiệu báo hiệu nhẹ (`{ "type": "sync_available", "latest_seq": 1043, "source_device_id": "..." }`), không truyền payload ghi chú; khi nhận tín hiệu (và `source_device_id != current_device_id`), `notes-sync` ở client tự động gọi `/v1/sync/pull`.

### 4.6. Auth & E2EE Key Management Endpoints
- `POST /v1/auth/salt`: Trả về `kdf_salt` và `kdf_params` cho email (có chống user enumeration bằng salt giả lập xác định khi email không tồn tại).
- `POST /v1/auth/register` & `POST /v1/auth/login`: Xác thực bằng `Auth Key (AK)` (server hash thêm 1 vòng Argon2id trước khi lưu/so sánh), cấp JWT + Refresh Token và trả về `key_bundle`.
- `GET /v1/keys/bundle`: Lấy `wrapped_amk`, `recovery_wrapped_amk`, `key_ring_encrypted`, `key_version`.
- `PUT /v1/auth/credentials`: Cập nhật nguyên tử `auth_verifier`, `kdf_salt`, `wrapped_amk` khi đổi mật khẩu và thu hồi toàn bộ Refresh Token của thiết bị khác.
- `PUT /v1/keys/rotate`: Xoay `AMK` (optimistic locking theo `expected_key_version`, lưu `key_ring_encrypted`). Chi tiết xem tại [07-e2ee-cryptography.md](./07-e2ee-cryptography.md).

---

## 5. Vận hành, Bảo mật & Mở rộng

- **Không phụ thuộc nhà cung cấp**: Chuyển đổi giữa self-host (Docker Compose với PostgreSQL + MinIO + Centrifugo) và Cloud (RDS + S3) chỉ bằng cách thay đổi biến môi trường (`SPRING_DATASOURCE_URL`, `S3_ENDPOINT`, `CENTRIFUGO_API_URL`).
- **Hiệu năng & Concurrency**: Spring Boot giữ vai trò REST API stateless thuần túy (Java 21 Virtual Threads tối ưu cho JDBC/S3 I/O), còn toàn bộ persistent connections và fan-out real-time do **Centrifugo** đảm nhận hiệu quả với chi phí tài nguyên rất thấp.
- **Bảo mật**:
  - Enforce TLS 1.3 ở reverse proxy (Nginx / Caddy / Traefik).
  - Rate limiting theo `user_id` / IP (Bucket4j).
  - Zero-knowledge: Server không bao giờ nắm giữ Master Sync Key hay Note Lock Key của người dùng.
