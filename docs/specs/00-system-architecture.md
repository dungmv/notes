# 00. Kiến trúc Tổng quan Toàn hệ thống (System Architecture)

Tài liệu này mô tả bức tranh tổng thể (End-to-End Architecture) của toàn bộ hệ thống ghi chú đa nền tảng, bao gồm các **Native Clients**, **Rust Shared Core**, và **Backend Services** (Spring Boot 4, PostgreSQL và S3/MinIO).

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
                      |  HTTPS REST (Sync / Auth)
                      v
================================================================================
                     BACKEND INFRASTRUCTURE (SELF-HOST / CLOUD)
================================================================================
                   [Reverse Proxy / TLS 1.3]
                               |
                               | HTTPS REST (client khởi tạo)
                               v
                   [Spring Boot 4 / Java 21+]
                      |                  |
              JDBC    |                  | S3 API
                      v                  v
              [PostgreSQL 16+]     [S3 / MinIO]
```

Client truyền blob trực tiếp với S3/MinIO qua Presigned URL do API cấp.

---

## 2. Phân tách Trách nhiệm (Separation of Concerns)

| Thành phần | Công nghệ chính | Trách nhiệm cốt lõi |
| :--- | :--- | :--- |
| **Native Clients** | SwiftUI (macOS/iOS), WinUI 3 (Windows), GTK4 (Linux), Compose (Android) | Render giao diện native, xử lý input/phím tắt, Rich Text Editor, tích hợp sinh trắc học (Touch ID / Windows Hello). Không chứa logic nghiệp vụ hay sync. |
| **Rust Core (`notes-core`)** | Rust + UniFFI + SQLite (WAL/FTS5) + CRDT (`yrs`) | Single Source of Truth cho toàn bộ nghiệp vụ: mô hình dữ liệu, CRDT merge, tìm kiếm tiếng Việt (FTS5), mã hóa E2EE, lưu trữ offline và Sync Engine. |
| **API Server** | Java 21+ (Virtual Threads) + Spring Boot 4.x | Stateless REST API: xác thực JWT/Device, nhận và trả delta mã hóa, xác nhận blob và cấp S3 Presigned URLs. |
| **Database** | PostgreSQL 16+ (Flyway) | Lưu tài khoản, thiết bị, bảng `sync_deltas` (append-only theo `server_seq`), và metadata deduplication của blob (`user_blobs`). |
| **Object Storage** | MinIO (self-host) hoặc AWS S3 / Backblaze B2 | Lưu trữ nhị phân các file đính kèm đã mã hóa (ảnh, PDF, audio) theo `content_hash`. Client upload/download trực tiếp qua Presigned URL. |

---

## 3. Luồng Dữ liệu Xuyên suốt (End-to-End Flows)

### 3.1. Luồng Soạn thảo Cục bộ (Offline-First)
1. Người dùng gõ nội dung hoặc thao tác trên UI (ví dụ: check một mục checklist trên macOS).
2. UI gửi `EditorOp` qua FFI vào `notes-core`.
3. UI có thể hiện thay đổi ngay. `notes-storage` commit tài liệu, dữ liệu dẫn xuất và outbox khi bật sync trong một transaction.
4. Core xác nhận đã lưu sau commit mà không chờ mạng. Xem [hợp đồng lưu](./02-data-model-and-storage.md#31-lưu-bền-vững-và-khôi-phục).

### 3.2. Luồng Đồng bộ Đa thiết bị

1. Thiết bị A upload blob đã mã hóa và xác nhận upload trước khi gửi delta tham chiếu blob.
2. A gọi `POST /v1/sync/push` với delta mã hóa. API cấp thứ tự theo tài khoản và commit PostgreSQL trước khi trả biên nhận theo từng thao tác.
3. Thiết bị B gọi `GET /v1/sync/pull` khi mở hoặc quay lại app, khi có mạng trở lại, theo lịch polling hoặc qua nút **Đồng bộ ngay**.
4. B xác thực và giải mã trang, commit dữ liệu và cursor cùng lúc rồi phát sự kiện UI. Chi tiết tại [hợp đồng sync](./04-sync-and-security.md#12-luồng-đồng-bộ-sync-loop).

---

## 4. Ranh giới Tin cậy & Bảo mật (Trust Boundaries)

Hệ thống áp dụng mô hình **Zero-Knowledge End-to-End Encryption (E2EE)**:

- **Vùng Tin cậy (Client Device)**:
  - Dữ liệu plaintext, khóa giải mã (`Master Sync Key`, `Note Lock Key`) và chỉ mục tìm kiếm FTS5 chỉ tồn tại ở phía client.
  - Khóa được bảo vệ bởi **macOS Keychain (Touch ID)**, **Windows DPAPI / Windows Hello**, hoặc **Android Keystore / Linux libsecret**.
- **Vùng Không Tin cậy (Network & Backend)**:
  - Mọi nội dung ghi chú (`crdt_state`), tiêu đề, tên thư mục và file đính kèm đều được mã hóa bằng **XChaCha20-Poly1305** trước khi rời khỏi `notes-core`.
  - Backend (Spring Boot, PostgreSQL và S3/MinIO) thấy ciphertext và metadata định tuyến, thứ tự, kích thước theo [threat model](./07-e2ee-cryptography.md#13-giảm-thiểu-metadata-what-server-sees-vs-never-sees).

---

## 5. Mô hình Triển khai (Deployment Topologies)

1. **Self-host đơn node cho Phase 3**. Docker Compose chạy API, PostgreSQL và MinIO sau reverse proxy TLS.
2. **Cloud và HA sau mốc kiểm chứng**.
   - **Spring Boot 4**: Chạy nhiều replicas stateless phía sau Load Balancer.
   - **PostgreSQL**: Managed Postgres (Primary + Read Replica nếu cần).
   - **Object Storage**: AWS S3 / Backblaze B2 / Cloudflare R2.
