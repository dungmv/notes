# 04. Đồng bộ hóa & Bảo mật

## 1. Kiến trúc Đồng bộ (Offline-First Sync Engine)

Mục tiêu: Dữ liệu luôn sẵn sàng offline, đồng bộ tự động ở chế độ nền, không khóa ứng dụng khi mất mạng, và không mất dữ liệu khi sửa đồng thời trên nhiều máy.

### 1.1. Sync Protocol (Backend-Agnostic)
`notes-sync` định nghĩa một trait `SyncTransport` để có thể cắm nhiều backend khác nhau:
- **Spring Boot Backend (Mặc định, không vendor lock-in)**: Sử dụng **Java 21+ / Spring Boot 4.x + PostgreSQL** (lưu sync deltas/metadata) + **S3-compatible Storage / MinIO** (lưu encrypted blobs) + **Centrifugo** (real-time push notification). Xem chi tiết tại [06-backend-spring-boot.md](./06-backend-spring-boot.md).
- **Object Storage / WebDAV** (tùy chọn serverless-less sync qua manifest + delta files).

### 1.2. Luồng Đồng bộ (Sync Loop)
1. **Local Mutation**: Mọi thay đổi trên `Folder`, `Note` (CRDT update delta), `Attachment` được ghi vào bảng `sync_oplog` với `seq` tăng dần.
2. **Push**:
   - Đọc các record chưa sync (`synced_at IS NULL`) trong `sync_oplog`.
   - Upload blob đính kèm trước (theo `content_hash`, bỏ qua nếu server đã có — deduplication).
   - Gửi batch CRDT deltas + metadata operations lên server kèm `client_sync_cursor`.
3. **Pull**:
   - Lấy các delta mới từ server kể từ `last_server_cursor`.
   - Áp dụng CRDT deltas vào `notes-crdt` (`yrs::Transaction::apply_update`). Nhờ tính chất giao hoán và kết hợp của CRDT, thứ tự nhận delta không làm sai lệch kết quả cuối cùng.
   - Phát sự kiện `NoteContentUpdated` / `NoteListInvalidated` qua FFI để UI tự cập nhật mượt mà.
4. **Compaction**: Định kỳ gộp các CRDT delta cũ thành snapshot mới để giữ dung lượng truyền tải nhỏ.

---

## 2. Mô hình Bảo mật & Mã hóa (Security & E2EE)

> **Đặc tả mật mã chi tiết** (Threat Model, Cipher Suite, Wire Format, AAD, Streaming Blobs, Recovery Key, Key Rotation và Kiểm thử bảo mật) được định nghĩa đầy đủ tại [07-e2ee-cryptography.md](./07-e2ee-cryptography.md). Dưới đây là tóm tắt tổng quan:

### 2.1. Phân tầng Bảo vệ
1. **At-Rest (Trên thiết bị)**:
   - Tùy chọn mã hóa toàn bộ SQLite database (kể cả chỉ mục FTS5) bằng **SQLCipher** với khóa lưu trong OS Secure Storage.
   - **Locked Notes**: Luôn được mã hóa bất kể cài đặt sync, sử dụng `Note Lock Key (NLK)` tách biệt khỏi `AMK` và không ghi plaintext vào FTS5.
2. **In-Transit**: Bắt buộc TLS 1.3 cho mọi kết nối sync.
3. **End-to-End Encryption (E2EE)**:
   - Toàn bộ `title`, `snippet`, `crdt_state`, `folder.name`, `tags`, và `attachments` đều được mã hóa bằng **XChaCha20-Poly1305** (kèm **AAD** ràng buộc `user_id`, `entity_id`, `client_op_id`, `key_version` và **STREAM AEAD** cho file đính kèm) tại client trước khi rời khỏi `notes-core`.
   - Server chỉ thấy `entity_id`, `server_seq`, `created_at`, `ciphertext` (đã padding), và `auth_tag`.

### 2.2. Quản lý Khóa (Key Management)

```text
User Passphrase
      │
      ▼  Argon2id (salt, m=64MB, t=3, p=1)
Master Secret (MS)
      │
      ├──► HKDF ──► Auth Key (AK) ──► Xác thực đăng nhập (Server không nhận MS/KEK)
      │
      └──► HKDF ──► Key Encryption Key (KEK)
                         │
                         ▼  Unwrap (hỗ trợ cả 256-bit Recovery Key)
                    Account Master Key (AMK) ──► Wrap từng Per-Entity DEK & Blob Keys

Locked Notes (độc lập):
Lock Passphrase / Biometric Secret ──► Note Lock Key (NLK) ──► Wrap Locked Note DEK
```

- **Tích hợp Biometrics của OS (Không lưu mật khẩu thô)**:
  - **macOS**: Lưu `KEK` hoặc `NLK` vào **macOS Keychain** với ACL `biometryCurrentSet` (yêu cầu **Touch ID** qua `LocalAuthentication`).
  - **Windows**: Dùng **Windows Hello (`UserConsentVerifier`)** để xác thực người dùng, sau đó giải mã khóa được bảo vệ bằng **Windows DPAPI / `Windows.Security.Credentials.KeyCredentialManager`**.
  - Rust Core **không tự gọi API sinh trắc học của OS**. Thay vào đó, Client Native thực hiện xác thực Touch ID / Windows Hello, lấy lại key material từ Secure Storage của OS, và truyền vào `notes-core` qua FFI (`unlock_with_key(bytes)`). Sau khi khóa note lại, `notes-core` gọi `zeroize` để xóa sạch key khỏi RAM.
