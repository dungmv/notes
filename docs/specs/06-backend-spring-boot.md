# 06. Backend Service (Java / Spring Boot)

Tài liệu này đặc tả kiến trúc backend cho tính năng đồng bộ đa thiết bị sử dụng **Java 21+ / Spring Boot 4.x**, hướng tới tính độc lập hạ tầng (**không vendor lock-in**): có thể tự host (self-hosted) bằng Docker Compose hoặc triển khai lên bất kỳ cloud/VPS nào.

---

## 1. Kiến trúc Tổng thể & Tech Stack

Phase 3 dùng Spring Boot, PostgreSQL và MinIO/S3 trên một node. Client chủ động gọi HTTPS REST cho mọi lượt đồng bộ.

```text
[Client notes-sync] -- HTTPS REST --> [Spring Boot 4] -- JDBC --> [PostgreSQL]
         |                                   |
         |                                   | S3 API
         |                                   v
         +----- HTTPS qua Presigned URL --> [S3 / MinIO]
```

### Thành phần Công nghệ
- **Runtime & Framework**: **Java 21+** (bật **Virtual Threads** `spring.threads.virtual.enabled=true`) + **Spring Boot 4.x** (Spring Web, Spring Security, Spring Data JDBC / jOOQ). Spring Boot hoàn toàn **stateless**, không giữ kết nối dài hạn.
- **Database**: **PostgreSQL 16+** kết hợp **Flyway** cho schema migrations.
- **Object Storage (Chuẩn S3)**: Giao tiếp qua chuẩn S3 API (`software.amazon.awssdk:s3` + `S3Presigner`), tương thích **MinIO** (self-host), AWS S3, Backblaze B2, hoặc Wasabi. Client upload/download file đính kèm trực tiếp bằng Presigned URL để giảm tải cho Spring Boot server.
- **Đóng gói**: Docker image (hỗ trợ cả **JVM** và **GraalVM Native Image** nếu muốn tối ưu RAM/startup) + `docker-compose.yml` chuẩn.

---

## 2. Cấu trúc Project Backend

```text
backend/
├── pom.xml (hoặc build.gradle.kts)
├── Dockerfile
├── docker-compose.yml                  # Spring Boot + PostgreSQL + MinIO
└── src/main/
    ├── resources/
    │   ├── application.yml
    │   └── db/migration/
    │       └── V1__init_schema.sql     # Flyway migration
    └── java/com/notes/backend/
        ├── common/                     # SecurityConfig, S3Config, GlobalExceptionHandler
        └── features/                   # Tổ chức theo feature (giống NestJS)
            ├── auth/                   # AuthController, AuthService, UserRepository, dto/, model/
            ├── sync/                   # SyncController, SyncService, SyncRepository, dto/, model/
            └── blob/                   # BlobController, BlobService, BlobRepository, dto/
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
    last_sync_seq BIGINT NOT NULL DEFAULT 0,
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
    server_seq BIGINT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id UUID NOT NULL,
    entity_type TEXT NOT NULL,         -- 'folder' | 'note' | 'attachment_meta'
    entity_id UUID NOT NULL,
    client_op_id UUID NOT NULL,        -- Idempotency key tránh trùng lặp khi retry
    crypto_version SMALLINT NOT NULL DEFAULT 1,
    key_version INT NOT NULL DEFAULT 1,
    encrypted_payload BYTEA NOT NULL,  -- Envelope ciphertext (CRDT delta hoặc folder metadata)
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, server_seq),
    UNIQUE (user_id, client_op_id)
);
CREATE INDEX idx_sync_deltas_entity ON sync_deltas(user_id, entity_id, server_seq);

-- Bảng theo dõi các blob đính kèm trên S3/MinIO (deduplication & dọn rác)
CREATE TABLE user_blobs (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,        -- blob_id (định danh opaque theo spec 07)
    byte_size BIGINT NOT NULL,
    storage_key TEXT NOT NULL,
    ciphertext_sha256 TEXT NOT NULL,
    ref_count INT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    PRIMARY KEY (user_id, content_hash)
);
```

---

## 4. API Endpoints & Giao thức Đồng bộ

Các endpoint đăng ký, lấy salt và đăng nhập dùng luồng xác thực riêng. Với endpoint dùng JWT, Spring Security kiểm tra `Authorization: Bearer <jwt>`, ràng buộc `X-Device-Id: <device_uuid>` với phiên đã xác thực và kiểm tra thiết bị còn hiệu lực. Khi thu hồi thiết bị, các yêu cầu mới bằng access token cũ bị từ chối và refresh token bị thu hồi.

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
- **Xử lý trên Spring Boot**.
  1. Xác thực batch và yêu cầu mỗi `client_op_id` chỉ xuất hiện một lần. Trong cùng một transaction, khóa dòng `users` của tài khoản bằng `SELECT … FOR UPDATE`. Mọi thao tác thêm delta phải đi qua khóa này.
  2. Đối chiếu `client_op_id` với delta đã lưu. Với retry cùng dữ liệu, trả lại số thứ tự đã cấp. Nếu ID trùng nhưng entity, phiên bản hoặc envelope khác, từ chối cả batch bằng HTTP 409. Mỗi delta mới lấy số tiếp theo từ `last_sync_seq`; chèn delta và cập nhật bộ đếm trong cùng transaction.
  3. Trả biên nhận sau commit.
- **Response** `{ "receipts": [{ "client_op_id": "uuid-v7", "server_seq": 1042 }] }`. Client dùng biên nhận để đánh dấu outbox theo từng ID, kể cả khi phản hồi đầu tiên bị mất.

### 4.2. Pull Deltas (`GET /v1/sync/pull?since_seq=1000&limit=200`)
- Truy vấn `sync_deltas` đã commit theo tài khoản, `server_seq > since_seq`, thứ tự tăng dần. Đọc tối đa `limit + 1` dòng trong cùng truy vấn để tính `has_more`, trả tối đa `limit` dòng.
- **Response** `{ "deltas": [...], "next_cursor": 1042, "has_more": false }`. Mỗi delta trả đủ metadata để tái tạo AAD, envelope và `server_seq`. `next_cursor` là số của delta cuối trang; trang rỗng trả lại `since_seq`.
- Client xác thực, áp dụng và lưu từng trang cùng cursor trước khi pull tiếp. Ca nghiệm thu bao gồm hai push đồng thời cùng tài khoản (push sau không thể công bố số lớn hơn trước khi push giữ khóa hoàn tất), phân trang, rollback và retry sau khi mất response.

### 4.3. Compaction

Phase 3 giữ log append-only. Trước khi hỗ trợ compaction, đặc tả cách chứng minh snapshot bao phủ các delta bị thay thế, phục hồi thiết bị mới hoặc offline lâu, giữ biên nhận phục vụ retry và xử lý viết đồng thời khi tạo snapshot.

### 4.4. Blob Upload / Download (`S3Presigner`)
- **`POST /v1/blobs/presign-upload`**:
  - Client gửi `{ content_hash, byte_size, ciphertext_sha256 }`. `content_hash` là ID blob; hai trường còn lại mô tả ciphertext.
  - Trả `exists: true` cho blob đã xác nhận có cùng kích thước và checksum. Nếu chưa xác nhận, cấp Presigned PUT URL trỏ đến object upload tạm, ký kèm checksum ciphertext. Token upload ràng buộc tài khoản, ID blob, kích thước và checksum.
- **`POST /v1/blobs/complete-upload`**. Client gửi token upload. Server xác minh object, kích thước và checksum, đưa object vào vị trí đã xác nhận và ghi `user_blobs` trước khi trả thành công. Retry cùng blob phải an toàn; cùng ID với ciphertext khác bị từ chối. URL upload tạm không ghi được vào object đã xác nhận. Client chỉ push delta tham chiếu blob sau bước này.
- **`GET /v1/blobs/presign-download?hash=<content_hash>`**:
  - Kiểm tra quyền sở hữu trong `user_blobs`, trả về Presigned GET URL.

### 4.5. Auth & E2EE Key Management Endpoints
- `POST /v1/auth/salt`: Trả về `kdf_salt` và `kdf_params` cho email (có chống user enumeration bằng salt giả lập xác định khi email không tồn tại).
- `POST /v1/auth/register` & `POST /v1/auth/login`: Xác thực bằng `Auth Key (AK)` (server hash thêm 1 vòng Argon2id trước khi lưu/so sánh), cấp JWT + Refresh Token và trả về `key_bundle`.
- `GET /v1/keys/bundle`: Lấy `wrapped_amk`, `recovery_wrapped_amk`, `key_ring_encrypted`, `key_version`.
- `PUT /v1/auth/credentials`: Cập nhật nguyên tử `auth_verifier`, `kdf_salt`, `wrapped_amk` khi đổi mật khẩu và thu hồi toàn bộ Refresh Token của thiết bị khác.
- `PUT /v1/keys/rotate`: Xoay `AMK` (optimistic locking theo `expected_key_version`, lưu `key_ring_encrypted`). Chi tiết xem tại [07-e2ee-cryptography.md](./07-e2ee-cryptography.md).

---

## 5. Vận hành, Bảo mật & Mở rộng

- **Cấu hình triển khai**. Self-host dùng Docker Compose với Spring Boot, PostgreSQL và MinIO. Cấu hình kết nối qua `SPRING_DATASOURCE_URL` và `S3_ENDPOINT` để dùng PostgreSQL và kho S3 tương thích khác.
- **Hiệu năng và concurrency**. Đo thời gian giữ khóa theo tài khoản, độ trễ pull và chi phí polling trên bản self-host.
- **Bảo mật**:
  - Enforce TLS 1.3 ở reverse proxy (Nginx / Caddy / Traefik).
  - Rate limiting theo `user_id` / IP (Bucket4j).
  - Zero-knowledge: Server không bao giờ nắm giữ Master Sync Key hay Note Lock Key của người dùng.
