# 00. Kiến trúc Tổng quan Toàn hệ thống (System Architecture)

Tài liệu này mô tả bức tranh tổng thể (End-to-End Architecture) của toàn bộ hệ thống ghi chú đa nền tảng, bao gồm các **Native Clients**, **Rust Shared Core**, và **Backend Services** (Spring Boot 4 + PostgreSQL + S3/MinIO + Centrifugo).

> Chi tiết bên trong Rust Core & FFI xem tại [01-architecture-and-ffi.md](./01-architecture-and-ffi.md); chi tiết Backend xem tại [06-backend-spring-boot.md](./06-backend-spring-boot.md).

---

## 1. Sơ đồ Kiến trúc Toàn hệ thống

```text
================================================================================
                        CLIENT DEVICES (OFFLINE-FIRST)
================================================================================
  +--------------------+   +--------------------+   +------------------------+
  |   macOS (SwiftUI)  |   | Windows (WinUI 3)  |   | Linux / iOS / Android  |
  |   AppKit/TextKit2  |   | C# WinAppSDK       |   | GTK4 / SwiftUI / Kotlin|
  +---------+----------+   +---------+----------+   +-----------+------------+
            | UniFFI                 | UniFFI C# / C-ABI        | Direct / UniFFI
            v                        v                          v
  +--------------------------------------------------------------------------+
  |                        notes-core (Rust Shared Core)                     |
  |  Domain | CRDT (yrs) | Crypto (E2EE) | FTS5 Search | Sync Engine         |
  +-------------------+----------------------------------+-------------------+
                      |                                  |
                      v                                  v
             [Local SQLite (WAL)]               [Local Blob Store]
                      |
======================|=========================================================
                      |  HTTPS REST (Sync / Auth) + Presigned S3 + Centrifugo
                      v
================================================================================
                     BACKEND INFRASTRUCTURE (SELF-HOST / CLOUD)
================================================================================
          +-------------------------------------------------------+
          |                  Reverse Proxy / TLS 1.3              |
          |               (Nginx / Caddy / Traefik / LB)          |
          +------+----------------------------------------+-------+
                 | HTTPS REST                             | Real-time (WS/SSE)
                 v                                        v
  +------------------------------+             +-----------------------------+
  |   Spring Boot 4 (Java 21+)   |  Publish    |         Centrifugo          |
  |  Stateless API Server        |────────────►| Real-time Messaging Server  |
  |  (Auth, Sync, S3 Presigner)  |  HTTP/gRPC  | (Channel: sync:user#<id>)   |
  +------+----------------+------+             +-----------------------------+
         |                |
         | JDBC           | S3 API (Presigned URLs)
         v                v
  +--------------+  +----------------------------------+
  | PostgreSQL 16|  |   S3-Compatible Object Storage   |
  | (Sync Deltas,|  |    (MinIO / AWS S3 / Backblaze)  |
  |  Users/Blobs)|  |      (Encrypted Attachments)     |
  +--------------+  +----------------------------------+
```

---

## 2. Phân tách Trách nhiệm (Separation of Concerns)

| Thành phần | Công nghệ chính | Trách nhiệm cốt lõi |
| :--- | :--- | :--- |
| **Native Clients** | SwiftUI (macOS/iOS), WinUI 3 (Windows), GTK4 (Linux), Compose (Android) | Render giao diện native, xử lý input/phím tắt, Rich Text Editor, tích hợp sinh trắc học (Touch ID / Windows Hello). Không chứa logic nghiệp vụ hay sync. |
| **Rust Core (`notes-core`)** | Rust + UniFFI + SQLite (WAL/FTS5) + CRDT (`yrs`) | Single Source of Truth cho toàn bộ nghiệp vụ: mô hình dữ liệu, CRDT merge, tìm kiếm tiếng Việt (FTS5), mã hóa E2EE, lưu trữ offline và Sync Engine. |
| **API Server** | Java 21+ (Virtual Threads) + Spring Boot 4.x | Stateless REST API: xác thực JWT/Device, cấp Centrifugo Connection Token, nhận/trả encrypted CRDT deltas (idempotent), compaction và cấp S3 Presigned URLs. |
| **Database** | PostgreSQL 16+ (Flyway) | Lưu tài khoản, thiết bị, bảng `sync_deltas` (append-only theo `server_seq`), và metadata deduplication của blob (`user_blobs`). |
| **Object Storage** | MinIO (self-host) hoặc AWS S3 / Backblaze B2 | Lưu trữ nhị phân các file đính kèm đã mã hóa (ảnh, PDF, audio) theo `content_hash`. Client upload/download trực tiếp qua Presigned URL. |
| **Real-time Push** | Centrifugo | Duy trì kết nối dài hạn với hàng chục nghìn thiết bị và broadcast tín hiệu `sync_available` nhẹ khi có thay đổi mới. |

---

## 3. Luồng Dữ liệu Xuyên suốt (End-to-End Flows)

### 3.1. Luồng Soạn thảo Cục bộ (Offline-First)
1. Người dùng gõ nội dung hoặc thao tác trên UI (ví dụ: check một mục checklist trên macOS).
2. UI gửi `EditorOp` qua FFI vào `notes-core`.
3. `notes-crdt` cập nhật trạng thái tài liệu trong RAM, sinh ra `UpdateDelta` nhị phân, đồng thời `notes-storage` lưu vào SQLite cục bộ (debounce 300–500ms), cập nhật chỉ mục FTS5 và ghi một dòng vào bảng `sync_oplog`.
4. Toàn bộ thao tác hoàn tất tức thì mà không cần chờ mạng.

### 3.2. Luồng Đồng bộ & Thông báo Real-time (Multi-Device)
```text
Device A (notes-sync)         Spring Boot 4          PostgreSQL       S3/MinIO        Centrifugo       Device B (notes-sync)
        │                           │                    │               │                │                     │
        │── 1. Presign Upload ─────►│                    │               │                │                     │
        │◄─ Presigned PUT URL ──────│                    │               │                │                     │
        │── 2. Upload Encrypted Blob────────────────────────────────────►│                │                     │
        │                           │                    │               │                │                     │
        │── 3. POST /v1/sync/push ─►│                    │               │                │                     │
        │   (Encrypted CRDT Deltas) │── 4. Batch Insert─►│               │                │                     │
        │                           │   (ON CONFLICT     │               │                │                     │
        │                           │    DO NOTHING)     │               │                │                     │
        │                           │── 5. Publish sync_available ───────────────────────►│                     │
        │◄─ 200 OK (ack_seq) ───────│                    │               │                │── 6. Push Notice ──►│
        │                           │                    │               │                │                     │
        │                           │◄─ 7. GET /v1/sync/pull?since_seq=...──────────────────────────────────────│
        │                           │── Encrypted Deltas ──────────────────────────────────────────────────────►│
        │                           │                    │               │                │                     │
        │                           │                    │               │                │   [Decrypt + CRDT   │
        │                           │                    │               │                │    Merge + UI Event]│
```

---

## 4. Ranh giới Tin cậy & Bảo mật (Trust Boundaries)

Hệ thống áp dụng mô hình **Zero-Knowledge End-to-End Encryption (E2EE)**:

- **Vùng Tin cậy (Client Device)**:
  - Dữ liệu plaintext, khóa giải mã (`Master Sync Key`, `Note Lock Key`) và chỉ mục tìm kiếm FTS5 chỉ tồn tại ở phía client.
  - Khóa được bảo vệ bởi **macOS Keychain (Touch ID)**, **Windows DPAPI / Windows Hello**, hoặc **Android Keystore / Linux libsecret**.
- **Vùng Không Tin cậy (Network & Backend)**:
  - Mọi nội dung ghi chú (`crdt_state`), tiêu đề, tên thư mục và file đính kèm đều được mã hóa bằng **XChaCha20-Poly1305 / AES-256-GCM** trước khi rời khỏi `notes-core`.
  - Backend (Spring Boot, PostgreSQL, S3/MinIO, Centrifugo) chỉ thấy `user_id`, `entity_id`, `server_seq` và dữ liệu mã hóa (`BYTEA` / encrypted blob).

---

## 5. Mô hình Triển khai (Deployment Topologies)

1. **Self-Hosted (Single Node / Docker Compose)**:
   - Một file `docker-compose.yml` khởi chạy 4 container: `notes-api` (Spring Boot 4), `postgres` (PostgreSQL 16), `minio` (S3-compatible), và `centrifugo`, đặt sau Caddy/Nginx tự động cấp TLS.
2. **Cloud / High-Availability**:
   - **Spring Boot 4**: Chạy nhiều replicas stateless phía sau Load Balancer.
   - **PostgreSQL**: Managed Postgres (Primary + Read Replica nếu cần).
   - **Object Storage**: AWS S3 / Backblaze B2 / Cloudflare R2.
   - **Centrifugo**: Chạy cluster (kết nối qua Redis/NATS engine nội bộ của Centrifugo khi cần scale lớn).
