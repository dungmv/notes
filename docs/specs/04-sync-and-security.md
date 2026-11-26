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

### 2.1. Phân tầng Bảo vệ
1. **At-Rest (Trên thiết bị)**:
   - Tùy chọn mã hóa toàn bộ SQLite database bằng **SQLCipher** (hoặc mã hóa ở tầng ứng dụng).
   - **Locked Notes**: Luôn được mã hóa bất kể cài đặt sync, sử dụng khóa riêng tách biệt khỏi dữ liệu thường.
2. **In-Transit**: Bắt buộc TLS 1.3 cho mọi kết nối sync.
3. **End-to-End Encryption (E2EE)**:
   - Khi bật E2EE, toàn bộ `title`, `snippet`, `crdt_state`, `folder.name`, và `attachments` đều được mã hóa bằng **XChaCha20-Poly1305** (hoặc **AES-256-GCM**) tại client trước khi rời khỏi `notes-core`.
   - Server chỉ thấy `entity_id`, `updated_at` (hoặc timestamp đã làm mờ), `ciphertext`, và `auth_tag`.

### 2.2. Quản lý Khóa (Key Management)

```text
User Passphrase
      │
      ▼  Argon2id (salt, m=64MB, t=3, p=1)
Key Encryption Key (KEK)
      │
      ├──► Unwrap Master Sync Key (MSK) ──► Mã hóa Note/Folder/Blob khi Sync
      │
      └──► Unwrap Note Lock Key (NLK)   ──► Mã hóa riêng từng Locked Note
```

- **Tích hợp Biometrics của OS (Không lưu mật khẩu thô)**:
  - **macOS**: Lưu `KEK` hoặc `NLK` vào **macOS Keychain** với ACL `biometryCurrentSet` (yêu cầu **Touch ID** qua `LocalAuthentication`).
  - **Windows**: Dùng **Windows Hello (`UserConsentVerifier`)** để xác thực người dùng, sau đó giải mã khóa được bảo vệ bằng **Windows DPAPI / `Windows.Security.Credentials.KeyCredentialManager`**.
  - Rust Core **không tự gọi API sinh trắc học của OS**. Thay vào đó, Client Native thực hiện xác thực Touch ID / Windows Hello, lấy lại key material từ Secure Storage của OS, và truyền vào `notes-core` qua FFI (`unlock_with_key(bytes)`). Sau khi khóa note lại, `notes-core` gọi `zeroize` để xóa sạch key khỏi RAM.
