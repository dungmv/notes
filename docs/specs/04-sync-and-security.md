# 04. Đồng bộ hóa & Bảo mật

## 1. Kiến trúc Đồng bộ (Offline-First Sync Engine)

Ghi chú vẫn dùng được khi mất mạng. Client chủ động đồng bộ qua HTTPS REST khi đang hoạt động hoặc khi người dùng yêu cầu. Lượt đồng bộ chạy bất đồng bộ với editor.

### 1.1. Sync Protocol (Backend-Agnostic)
`notes-sync` định nghĩa một trait `SyncTransport` để có thể cắm nhiều backend khác nhau:
- **Spring Boot Backend (Mặc định, không vendor lock-in)**: Sử dụng **Java 21+ / Spring Boot 4.x + PostgreSQL** (lưu sync deltas/metadata) + **S3-compatible Storage / MinIO** (lưu encrypted blobs). Xem chi tiết tại [06-backend-spring-boot.md](./06-backend-spring-boot.md).
- **Object Storage / WebDAV** (tùy chọn serverless-less sync qua manifest + delta files).

### 1.2. Luồng Đồng bộ (Sync Loop)

Client thực hiện push rồi pull trong cùng lượt đồng bộ. Push gửi các thay đổi cục bộ đã commit. Pull đọc các thay đổi đã commit trên server.

1. **Local mutation**. Lưu thay đổi tài liệu cùng outbox trong [transaction cục bộ](./02-data-model-and-storage.md#31-lưu-bền-vững-và-khôi-phục). `seq` trong outbox là thứ tự cục bộ.
2. **Push**. Đọc các mục chưa đồng bộ, bảo đảm mọi blob được tham chiếu đã upload và xác nhận, rồi gửi delta với `client_op_id` ổn định. Khi nhận phản hồi, đánh dấu từng mục tương ứng với biên nhận; giữ mục chưa được xác nhận để retry.
3. **Pull**. Lấy từng trang theo cursor, xác thực và áp dụng toàn bộ trang. Commit trạng thái và `last_server_cursor` trong cùng transaction cục bộ, rồi mới phát sự kiện UI. Nếu xác thực hoặc lưu thất bại, giữ nguyên cursor và báo lỗi đồng bộ.
4. **Hội tụ**. Kiểm tra từng loại thao tác ngoài nội dung CRDT, gồm di chuyển, ghim, xóa và khôi phục. Phải định nghĩa quy tắc merge trước khi đưa từng loại thao tác vào sync. Dùng fixture riêng để kiểm tra hội tụ và bảo toàn các chỉnh sửa độc lập.
5. **Compaction**. Phase 3 giữ log append-only để thiết bị mới hoặc offline lâu có thể đọc lại. Chỉ bật dọn log khi có hợp đồng snapshot, cursor và phục hồi đã kiểm thử.

### 1.3. Lịch đồng bộ trên client

- **Kích hoạt ngay** khi mở app, khi cửa sổ trở lại hoạt động hoặc khi có mạng trở lại trong lúc app đang dùng. Sau một batch sửa đã lưu cục bộ, lên lịch sync sớm để gửi outbox.
- **Polling** khi app desktop đang hoạt động và đã bật sync. Thử mặc định mỗi **15 giây** rồi điều chỉnh theo độ trễ và số request đo được. Tạm dừng polling định kỳ khi app không hoạt động hoặc mất mạng. Retry lỗi mạng tạm thời dùng backoff.
- **Đồng bộ ngay** trên thanh công cụ macOS và Windows gọi `trigger_sync()` để gửi outbox và kéo các trang mới. Nếu mất mạng, hiển thị **Chờ mạng** và giữ các thay đổi đã lưu để gửi khi kết nối lại. UI phân biệt **Đã lưu trên thiết bị** với trạng thái đồng bộ.
- **Một lượt sync tại một thời điểm** cho mỗi tài khoản trên mỗi client. Các trigger trùng nhau dùng chung lượt đang chạy. Nếu có thay đổi được commit sau khi lượt hiện tại đã chốt phần outbox cần gửi, lên lịch lượt tiếp theo. Báo hoàn tất qua `SyncStateChanged` sau khi áp dụng trang cuối và xác nhận phần outbox thuộc lượt đó.
- **Mobile** lấy lúc app trở lại foreground làm điểm kích hoạt chính. Tác vụ đồng bộ nền phụ thuộc lịch của hệ điều hành.

---

## 2. Mô hình Bảo mật & Mã hóa (Security & E2EE)

> **Đặc tả mật mã chi tiết** (Threat Model, Cipher Suite, Wire Format, AAD, Streaming Blobs, Recovery Key, Key Rotation và Kiểm thử bảo mật) được định nghĩa đầy đủ tại [07-e2ee-cryptography.md](./07-e2ee-cryptography.md). Dưới đây là tóm tắt tổng quan:

### 2.1. Phân tầng Bảo vệ
1. **At-Rest (Trên thiết bị)**:
   - Dùng **SQLCipher** cho kho có bật Locked Notes (kể cả FTS5 và WAL), tùy chọn cho kho ghi chú thường. Khóa lưu trong OS Secure Storage.
   - **Locked Notes**. Dùng `Note Lock Key (NLK)` tách khỏi `AMK`. Tiêu đề là một phần của bản chiếu được phép hiện; FTS5 và cache tuân theo [chính sách lưu trữ](./02-data-model-and-storage.md#34-chính-sách-lưu-trữ-locked-notes).
2. **In-Transit**: Bắt buộc TLS 1.3 cho mọi kết nối sync.
3. **End-to-End Encryption (E2EE)**:
   - Toàn bộ `title`, `snippet`, `crdt_state`, `folder.name`, `tags`, và `attachments` đều được mã hóa bằng **XChaCha20-Poly1305** (kèm **AAD** ràng buộc `user_id`, `entity_id`, `client_op_id`, `key_version` và **STREAM AEAD** cho file đính kèm) tại client trước khi rời khỏi `notes-core`.
   - Server thấy ciphertext và metadata theo [mô hình đe dọa](./07-e2ee-cryptography.md#13-giảm-thiểu-metadata-what-server-sees-vs-never-sees).

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
