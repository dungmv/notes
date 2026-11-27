# 07. Đặc tả Mã hóa Đầu cuối (End-to-End Encryption — E2EE)

Tài liệu này đặc tả chi tiết kiến trúc mật mã, mô hình đe dọa, phân cấp khóa, định dạng gói tin mã hóa, xác thực Zero-Knowledge và quy trình quản lý vòng đời khóa cho `notes-crypto` và toàn bộ hệ thống đồng bộ.

> Xem kiến trúc tổng thể tại [00-system-architecture.md](./00-system-architecture.md), luồng đồng bộ tại [04-sync-and-security.md](./04-sync-and-security.md), và schema backend tại [06-backend-spring-boot.md](./06-backend-spring-boot.md).

---

## 1. Mô hình Đe dọa & Mục tiêu Bảo mật (Threat Model)

### 1.1. Phạm vi Bảo vệ (In-Scope Adversaries)
Hệ thống E2EE được thiết kế theo mô hình **Zero-Knowledge** nhằm bảo vệ dữ liệu người dùng trước các kịch bản tấn công sau:

1. **Server bị xâm nhập hoàn toàn (Compromised Backend / Storage)**:
   - Kẻ tấn công đọc được toàn bộ PostgreSQL, S3/MinIO, log hệ thống và lưu lượng API sync (**Honest-but-Curious**).
   - Kẻ tấn công chủ động sửa đổi, tráo đổi ciphertext giữa các ghi chú/người dùng, hoặc phát lại gói tin cũ (**Active Malicious Server**).
2. **Nghe lén & Chặn giữa đường (Network MITM)**:
   - Mọi kết nối đều bắt buộc TLS 1.3; lớp E2EE cung cấp thêm tầng bảo vệ độc lập ngay cả khi TLS bị hạ cấp hoặc bị chấm dứt tại proxy không tin cậy.
3. **Tấn công dò mật khẩu ngoại tuyến (Offline Brute-Force)**:
   - Ngay cả khi bảng `users` bị đánh cắp, chi phí dò mật khẩu bị đẩy lên rất cao nhờ **Argon2id** và việc tách biệt hoàn toàn khóa xác thực khỏi khóa mã hóa.
4. **Mất hoặc bị đánh cắp thiết bị (Lost / Stolen Device)**:
   - Dữ liệu cục bộ được bảo vệ ở tầng đĩa (**SQLCipher** / OS Full-Disk Encryption) và các ghi chú nhạy cảm (**Locked Notes**) yêu cầu xác thực sinh trắc học/mật khẩu riêng, không lưu plaintext vào FTS5.

### 1.2. Ngoài Phạm vi (Out-of-Scope)
- Thiết bị người dùng đã bị nhiễm malware/keylogger có quyền đọc trực tiếp bộ nhớ tiến trình khi ứng dụng đang mở khóa.
- Tấn công kênh kề vật lý (physical side-channel / cold boot attack) trên thiết bị đang mở khóa.

### 1.3. Giảm thiểu Metadata (What Server Sees vs. Never Sees)

| Thành phần | Server / S3 **THẤY** | Server / S3 **KHÔNG BAO GIỜ THẤY** |
| :--- | :--- | :--- |
| **Tài khoản** | `user_id`, `email_hash`, `auth_verifier`, `kdf_salt`, `wrapped_amk` | Mật khẩu gốc, `Master Secret`, `KEK`, `AMK`, `Recovery Key` |
| **Ghi chú & Thư mục** | `entity_id` (UUIDv7), `entity_type`, `server_seq`, `created_at`, kích thước ciphertext (đã padding) | `title`, `snippet`, `crdt_state`, `folder.name`, `parent_id`, `tags`, `smart_query`, quan hệ Note ↔ Folder |
| **File đính kèm** | `blob_id` (keyed hash), `byte_size` (ciphertext), `storage_key` | Nội dung file, `file_name`, `mime_type`, `thumbnail`, `ocr_text`, hay 2 user có dùng chung file không |

---

## 2. Bộ Thuật toán Mật mã Chuẩn hóa (Cryptographic Primitives)

Mọi hoạt động mật mã được đóng gói duy nhất trong crate **`notes-crypto`**. Tuyệt đối **không tự viết thuật toán mật mã**; chỉ sử dụng các crate đã được kiểm định từ hệ sinh thái **RustCrypto**.

| Vai trò | Thuật toán chuẩn (`Suite v1`) | Tham số bắt buộc | Rust Crate khuyến nghị |
| :--- | :--- | :--- | :--- |
| **Password KDF** | **Argon2id** (RFC 9106) | `m = 64 MiB` (65536 KiB), `t = 3`, `p = 1`, salt = `32 B`, out = `32 B` | `argon2` |
| **Subkey KDF** | **HKDF-SHA256** (RFC 5869) | Domain separation qua chuỗi `info` cố định | `hkdf`, `sha2` |
| **AEAD (Payload & Key Wrap)** | **XChaCha20-Poly1305** | Key = `32 B`, Nonce = `24 B` (random CSPRNG), Tag = `16 B` | `chacha20poly1305` |
| **Streaming AEAD (Blobs)** | **STREAM-XChaCha20-Poly1305** | Chunk = `64 KiB`, Base Nonce = `19 B` + Counter `4 B` + Flag `1 B` | `chacha20poly1305` (STREAM) |
| **Key Exchange (Future Share)** | **X25519** (RFC 7748) | 32-byte keys | `x25519-dalek` |
| **Digital Signature (Devices)** | **Ed25519** (RFC 8032) | 32-byte key, 64-byte signature | `ed25519-dalek` |
| **CSPRNG** | OS Entropy Source | `getrandom` / `OsRng` | `rand_core` |
| **Memory Zeroing** | Explicit Zeroization | Tự động xóa khóa khi `Drop` | `zeroize` |

### Lý do chọn XChaCha20-Poly1305 làm mặc định
1. **An toàn Nonce ở môi trường Offline-First đa thiết bị**: Nonce dài 192-bit (24 bytes) cho phép sinh hoàn toàn ngẫu nhiên trên nhiều thiết bị offline cùng lúc mà xác suất đụng độ gần như bằng 0 (không cần đồng bộ bộ đếm nonce như AES-GCM 96-bit).
2. **Hiệu năng đồng đều & Chống Timing Attack**: Chạy tốc độ cao bằng phần mềm thuần (constant-time) ngay cả trên thiết bị ARM/mobile hoặc CPU không hỗ trợ AES-NI.
3. **Crypto-Agility**: Mọi gói tin đều mang trường `crypto_version` và `cipher_id` để có thể nâng cấp bộ thuật toán (ví dụ: hậu lượng tử — Post-Quantum hybrid KEM) trong tương lai mà vẫn giải mã được dữ liệu cũ.

---

## 3. Hệ thống Phân cấp Khóa (Key Hierarchy)

```text
User Passphrase (không bao giờ rời khỏi client)
      │
      ▼  Argon2id(passphrase, kdf_salt, kdf_params)
Master Secret (MS, 32B) ──► [Zeroize ngay sau khi derive]
      │
      ├──► HKDF-Expand(MS, "notes-e2ee-v1:auth-key")
      │         └──► Auth Key (AK, 32B) ──► Gửi xác thực tới Server (Server hash thêm Argon2id)
      │
      └──► HKDF-Expand(MS, "notes-e2ee-v1:kek")
                └──► Key Encryption Key (KEK, 32B)
                          │
                          ▼  Unwrap (XChaCha20-Poly1305)
                 Account Master Key (AMK, 32B random CSPRNG)
                 ▲ (cũng được wrap bởi Recovery KEK từ 256-bit Recovery Key)
                 │
                 ├──► HKDF-Expand(AMK, "notes-e2ee-v1:entity-wrap")
                 │         └──► Entity Wrap Key (EWK) ──► Wrap/Unwrap từng Per-Entity DEK
                 │                                            │
                 │                                            ▼
                 │                                       Entity DEK (32B random / Note hoặc Folder)
                 │                                       └──► Mã hóa CRDT deltas & metadata
                 │
                 ├──► HKDF-Expand(AMK, "notes-e2ee-v1:blob-index")
                 │         └──► Blob Index Key (BIK) ──► Tính blob_id = HMAC(BIK, SHA256(plaintext))
                 │
                 └──► HKDF-Expand(AMK, "notes-e2ee-v1:blob-key")
                           └──► Blob Key Derivation ──► Dẫn xuất hoặc wrap Blob Encryption Key (BEK)

Locked Notes (Nhánh cách ly độc lập):
Lock Passphrase (hoặc Biometric-protected secret từ OS Keychain/DPAPI)
      │
      ▼  Argon2id(lock_passphrase, lock_salt)
Note Lock Key (NLK, 32B) ──► Wrap/Unwrap Locked Note DEK (tách biệt hoàn toàn khỏi AMK)
```

### 3.1. Giải thích Vai trò từng Lớp Khóa
- **`Master Secret (MS)`**: Chỉ tồn tại thoáng qua trong RAM lúc đăng nhập/đổi mật khẩu, sau đó bị `zeroize` ngay.
- **`Auth Key (AK)` vs `KEK`**: Tách biệt miền mật mã (domain separation). Ngay cả khi server biết `AK` trong quá trình đăng nhập, server **không thể suy ngược** ra `MS` hay `KEK`.
- **`Account Master Key (AMK)`**: Khóa gốc ngẫu nhiên 256-bit của tài khoản.
  - Được lưu trên server dưới dạng `wrapped_amk = AEAD(KEK, AMK, AAD=user_id || "amk" || key_version)`.
  - **Lợi ích**: Khi người dùng đổi mật khẩu, client chỉ cần re-wrap `AMK` bằng `KEK` mới — **không cần mã hóa lại toàn bộ hàng nghìn ghi chú**.
- **Per-Entity `DEK` (Data Encryption Key)**:
  - Mỗi `Note` và `Folder` có một `DEK` 256-bit ngẫu nhiên riêng.
  - `wrapped_dek` được đính kèm trong bản ghi khởi tạo/metadata của entity đó (được wrap bằng `EWK` nếu là ghi chú thường, hoặc bằng `NLK` nếu là Locked Note).
  - **Lợi ích**:
    1. Cô lập phạm vi ảnh hưởng (cryptographic isolation).
    2. Chuyển đổi Note thường ↔ Locked Note chỉ cần re-wrap/re-key đúng 1 ghi chú đó.
    3. Sẵn sàng mở rộng tính năng **Shared Notes** (chia sẻ `DEK` của 1 ghi chú qua X25519 cho người khác mà không lộ `AMK`).
- **`Recovery Key` (Khóa Khôi phục 256-bit)**:
  - Sinh ngẫu nhiên khi khởi tạo E2EE, hiển thị cho người dùng một lần duy nhất dưới dạng **24 từ BIP-39** hoặc chuỗi Base32 kèm checksum (ví dụ: `XXXX-XXXX-XXXX-XXXX-...`), khuyến khích lưu vào Password Manager hoặc in giấy.
  - Client tính `Recovery KEK = HKDF-Expand(RecoveryKey, "notes-e2ee-v1:recovery-kek")` và lưu thêm `recovery_wrapped_amk` lên server.
  - Khi quên mật khẩu, mục tiêu của luồng phục hồi là dùng `Recovery Key` để unwrap `AMK` ở client rồi thiết lập mật khẩu mới. Điều kiện nghiệm thu Phase 3 là chốt cách lấy bundle và đổi thông tin đăng nhập khi đã mất mật khẩu, như ghi tại TD-005 trong [sổ nợ kỹ thuật](../technical-debt.md).

---

## 4. Định dạng Gói tin Mã hóa & Ràng buộc Ngữ cảnh (Wire Format & AAD)

### 4.1. Binary Envelope Format (cho `sync_deltas` & Local Locked State)

Mọi `encrypted_payload` lưu trong `sync_deltas` (PostgreSQL) hoặc `crdt_state` của Locked Note tuân thủ định dạng nhị phân cố định (Big-Endian):

```text
+---------------+-------------+-----------------+---------------+--------------------+---------------+
| crypto_ver(1B)| cipher_id(1)| key_version (4B)|  nonce (24B)  | ciphertext (N B)   | auth_tag (16B)|
+---------------+-------------+-----------------+---------------+--------------------+---------------+
|     0x01      |    0x01     |    uint32 BE    |  random CSPRNG| XChaCha20(padded)  |  Poly1305     |
+---------------+-------------+-----------------+---------------+--------------------+---------------+
```

- **Overhead cố định**: `1 + 1 + 4 + 24 + 16 = 46 bytes`.
- **`crypto_ver`**: `0x01` (v1).
- **`cipher_id`**: `0x01` = XChaCha20-Poly1305.
- **`key_version`**: Số phiên bản của `AMK` (hoặc `NLK`) đã dùng để wrap/derive khóa, phục vụ Key Rotation.

### 4.2. Associated Authenticated Data (AAD) — Chống Tráo đổi Ciphertext

Để ngăn chặn một server độc hại copy ciphertext của Note A sang Note B, đổi `entity_type`, hoặc chuyển dữ liệu từ User A sang User B, **mọi phép mã hóa AEAD bắt buộc phải truyền AAD**:

```text
AAD = len_u32(context) || context
      || u8(crypto_ver) || u8(cipher_id) || u32_be(key_version)
      || uuid_bytes(user_id)
      || len_u32(entity_type) || entity_type
      || uuid_bytes(entity_id)
      || uuid_bytes(client_op_id)
```

- Với `context = "notes-sync-delta-v1"` cho sync deltas, `"notes-amk-wrap-v1"` cho wrapped AMK, `"notes-locked-local-v1"` cho Locked Note cục bộ.
- Nếu bất kỳ byte nào trong `user_id`, `entity_id`, `entity_type`, `client_op_id`, `key_version` hoặc `ciphertext` bị thay đổi, bước giải mã sẽ lập tức thất bại với `CryptoError::AuthenticationFailed` và từ chối áp dụng vào CRDT.

### 4.3. Plaintext Padding (Chống Rò rỉ Độ dài)

Để giảm thiểu phân tích lưu lượng (traffic analysis) suy đoán độ dài tiêu đề hoặc ký tự gõ phím từ kích thước `sync_deltas`:
- Trước khi mã hóa, plaintext được nén (tùy chọn `zstd` ở mức thấp cho snapshot lớn — *lưu ý*: không nén kèm dữ liệu do attacker kiểm soát để tránh CRIME/BREACH) và **pad theo bucket size**:
  - Payload ≤ 256 B → pad lên 256 B.
  - Payload ≤ 1 KiB → pad lên bội số 256 B.
  - Payload > 1 KiB → pad lên bội số 1 KiB.
- Định dạng nội bộ plaintext trước khi mã hóa: `u32_be(plaintext_len) || plaintext_bytes || zero_padding`.

---

## 5. Mã hóa File Đính kèm (Encrypted Blobs & Safe Deduplication)

File đính kèm (ảnh, PDF, audio) có thể lớn hàng chục đến hàng trăm MB nên **không mã hóa một lần trong RAM** mà dùng **STREAM AEAD** (chunk 64 KiB):

### 5.1. Định dạng Blob Stream
- **Header (32 bytes)**: `magic("NTBL", 4B) || version(1B=0x01) || cipher_id(1B=0x01) || chunk_size_log2(1B=16) || reserved(6B) || base_nonce(19B)`.
- **Chunks**: Với mỗi chunk `i` (`u32`, bắt đầu từ 0):
  - `chunk_nonce (24B) = base_nonce (19B) || u32_be(i) || flag (1B: 0x00 cho chunk thường, 0x01 cho chunk cuối)`.
  - `chunk_aad = header_bytes || u32_be(i) || flag`.
  - Mã hóa bằng XChaCha20-Poly1305 → xuất `ciphertext_chunk || tag (16B)`.
- Cơ chế này chống hoàn toàn tấn công cắt đuôi (truncation), đảo thứ tự chunk (reordering) và chèn chunk giả, đồng thời hỗ trợ **streaming upload/download** với RAM tiêu thụ < 256 KiB.

### 5.2. Deduplication An toàn (Per-User Keyed Convergent Encryption)
- **Ghi chú thường**:
  - `plaintext_hash = SHA256(plaintext_file)`.
  - `blob_id = HMAC-SHA256(BIK, plaintext_hash)` (gửi lên server làm `content_hash` trong `user_blobs`).
  - `BEK = HKDF-SHA256(AMK, salt = plaintext_hash, info = "notes-e2ee-v1:blob-key")`. Metadata attachment bên trong delta đã mã hóa chứa `blob_id`, `plaintext_hash` và phiên bản AMK dùng để tính khóa blob. Thiết bị nhận dẫn xuất BEK từ metadata đó trước khi tải và giải mã.
  - `base_nonce` dẫn xuất xác định từ `HKDF-SHA256(BEK, info = "notes-e2ee-v1:blob-nonce")` (chỉ dùng duy nhất cho đúng `plaintext_hash` đó).
  - **Kết quả**: Trùng lặp file trong cùng 1 tài khoản vẫn được deduplicate 100% trên S3/MinIO, nhưng **2 tài khoản khác nhau upload cùng 1 file sẽ có `blob_id` và `ciphertext` hoàn toàn khác nhau** (ngăn chặn cross-user confirmation attack).
- **Locked Notes**:
  - Luôn dùng `BEK` và `base_nonce` **ngẫu nhiên hoàn toàn** (không deduplicate) và lưu `BEK` bên trong CRDT đã mã hóa của Locked Note.

---

## 6. Xác thực Zero-Knowledge (Authentication Flow)

1. **Đăng ký (Registration)**:
   - Client sinh `kdf_salt (32B)` và `AMK (32B)` ngẫu nhiên.
   - Client tính `MS = Argon2id(passphrase, kdf_salt, kdf_params)`.
   - Client dẫn xuất `AK` và `KEK` từ `MS` bằng HKDF.
   - Client mã hóa `AMK` bằng `KEK` → `wrapped_amk` (và tùy chọn `recovery_wrapped_amk`).
   - Client gửi `{ email, auth_key: AK, kdf_salt, kdf_params, wrapped_amk, recovery_wrapped_amk }` lên `POST /v1/auth/register` qua TLS 1.3.
   - Server **hash `AK` thêm 1 vòng Argon2id** (với salt riêng của server) rồi lưu vào `users.auth_verifier`. Server không bao giờ lưu `AK` thô.
2. **Đăng nhập (Login)**:
   - Bước 1: Client gọi `POST /v1/auth/salt` kèm `email` để lấy `{ kdf_salt, kdf_params }`.
     - *Chống User Enumeration*: Nếu email không tồn tại, server trả về `kdf_salt` giả lập xác định bằng `HMAC-SHA256(server_pepper, email)` và `kdf_params` mặc định để kẻ tấn công không phân biệt được tài khoản tồn tại hay chưa.
   - Bước 2: Client tính `MS`, `AK`, `KEK` tại máy, gửi `{ email, auth_key: AK, device_info }` tới `POST /v1/auth/login`.
   - Bước 3: Server xác minh `AK` với `auth_verifier`, cấp cặp `access_token` (JWT ngắn hạn 15 phút) + `refresh_token` (rotation) và trả về `key_bundle` (`wrapped_amk`, `key_version`).
   - Bước 4: Client dùng `KEK` giải mã `wrapped_amk` lấy `AMK`, lưu `AMK` vào OS Secure Storage (Keychain/DPAPI), và `zeroize` `MS`, `KEK`, `AK`.

> *Lộ trình nâng cấp (Phase 4+)*: Có thể thay bước gửi `AK` qua TLS bằng giao thức **OPAQUE (RFC 9807 / aPAKE)** để ngay cả TLS termination proxy cũng không thấy `AK`.

---

## 7. Bảo mật Cục bộ (At-Rest), Biometrics & Vệ sinh Bộ nhớ

### 7.1. Mã hóa Cơ sở Dữ liệu Cục bộ
- **SQLCipher**. Tùy chọn với kho ghi chú thường. Kho hỗ trợ khóa ghi chú phải dùng SQLCipher, kể cả WAL, trước khi bật Locked Notes. Khóa cục bộ sinh ngẫu nhiên và lưu trong OS Secure Storage. Với kho SQLite cũ, cần có quy trình chuyển đổi và xử lý file DB, WAL và bản sao chưa mã hóa trước khi bật tính năng.
- **Locked Notes (Bảo vệ kép)**:
  - Dù ứng dụng đã mở khóa bằng `AMK`, các Locked Notes vẫn giữ `crdt_state` ở dạng ciphertext (mã hóa bằng `NLK`).
  - Tiêu đề dùng được trong danh sách và tìm theo tiêu đề. FTS5, snippet, tag và metadata attachment dạng rõ tuân theo [chính sách lưu trữ](./02-data-model-and-storage.md#34-chính-sách-lưu-trữ-locked-notes), kể cả trong phiên mở khóa.
  - Tự động khóa lại (xóa `NLK` và plaintext khỏi RAM) khi: bấm *Lock Now*, hết thời gian chờ (mặc định 5 phút không tương tác), app xuống nền (mobile), hoặc OS khóa màn hình/sleep.

Phase 3 đồng bộ E2EE cho ghi chú thường. Mốc Locked Notes cục bộ chỉ mở tính năng trong thư viện local-only. Nếu thư viện đã có ghi chú khóa, việc bật sync cần đợi đến khi có hỗ trợ Locked Notes qua sync. Trước mốc đó, cần chốt và kiểm thử cách chuyển note thường đã sync sang trạng thái khóa, gồm thay khóa nội dung, xử lý log/snapshot cũ và thiết bị còn offline.

### 7.2. Tích hợp OS Secure Storage & Biometrics
`notes-core` **không tự gọi API sinh trắc học**. Client Native chịu trách nhiệm mở khóa kho bảo mật của hệ điều hành và truyền key material qua FFI:

| Nền tảng | Cơ chế lưu trữ khóa (`AMK`, `Local DB Key`, `NLK`) | Ràng buộc sinh trắc học |
| :--- | :--- | :--- |
| **macOS / iOS** | Keychain (`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`) | `SecAccessControl` với `.biometryCurrentSet` (Touch ID / Face ID) |
| **Windows** | DPAPI / `Windows.Security.Credentials.KeyCredentialManager` | `UserConsentVerifier` (Windows Hello) |
| **Linux** | `libsecret` (GNOME Keyring / KWallet) | Session unlock / Polkit |
| **Android** | Android Keystore (TEE / StrongBox hardware-backed) | `BiometricPrompt` (`CryptoObject`) |

### 7.3. Vệ sinh Bộ nhớ trong Rust (`notes-crypto`)
- Mọi kiểu khóa (`MasterSecret`, `KeyEncryptionKey`, `AccountMasterKey`, `DataEncryptionKey`, `NoteLockKey`) bắt buộc bọc trong `zeroize::Zeroizing<[u8; 32]>` hoặc `#[derive(Zeroize, ZeroizeOnDrop)]`.
- **Cấm implement `Debug` in ra raw bytes** cho bất kỳ struct khóa hay plaintext nhạy cảm nào (tránh lộ trong log hoặc panic backtrace); chỉ in `"[REDACTED]"`.
- Trên FFI boundary (`notes-ffi`), buffer chứa secret truyền từ Swift/C#/Kotlin vào Rust phải được xóa (`zeroize`) ngay sau khi nạp xong.

---

## 8. Vòng đời Khóa (Key Lifecycle: Password Change, Rotation & Revocation)

### 8.1. Đổi Mật khẩu (Password Change)
1. Client xác thực mật khẩu cũ và có sẵn `AMK` hợp lệ trong RAM.
2. Sinh `kdf_salt_new` (32B), tính `MS_new`, `AK_new`, `KEK_new`.
3. Re-wrap `AMK` bằng `KEK_new` → `wrapped_amk_new`.
4. Gửi request nguyên tử `PUT /v1/auth/credentials` kèm `current_auth_key`, `new_auth_key`, `new_kdf_salt`, `new_kdf_params`, `new_wrapped_amk`.
5. Server cập nhật trong 1 transaction và **thu hồi toàn bộ Refresh Token của các thiết bị khác**, buộc đăng nhập lại bằng mật khẩu mới.

### 8.2. Xoay Khóa Khẩn cấp (AMK Rotation)
Áp dụng khi người dùng nghi ngờ thiết bị bị mất ở trạng thái đã mở khóa:
1. Thu hồi thiết bị bị mất (`DELETE /v1/devices/{id}`).
2. Client sinh `AMK_v(n+1)` mới, tăng `key_version = n + 1`.
3. Wrap `AMK_v(n)` cũ bằng `AMK_v(n+1)` và lưu vào **Key Ring** (để các thiết bị hợp lệ vẫn giải mã được các delta lịch sử chưa compact).
4. Đẩy `wrapped_amk` mới + Key Ring đã mã hóa lên `PUT /v1/keys/rotate`.
5. Chạy tác vụ nền (background compaction & re-encryption) để sinh snapshot mới mã hóa bằng `AMK_v(n+1)` cho toàn bộ ghi chú và xóa các delta cũ trên server.

---

## 9. Bổ sung Backend Schema & API cho E2EE

Để hỗ trợ đầy đủ đặc tả này, `06-backend-spring-boot.md` bổ sung các cột và endpoint sau:

### 9.1. Bổ sung vào PostgreSQL Schema
- Bảng `users`:
  - `kdf_params_json JSONB NOT NULL` (lưu `{ "alg": "argon2id", "m_kib": 65536, "t": 3, "p": 1 }`).
  - `wrapped_amk BYTEA NOT NULL` (envelope mã hóa `AMK` bằng `KEK`).
  - `recovery_wrapped_amk BYTEA` (envelope mã hóa `AMK` bằng `Recovery KEK`).
  - `key_ring_encrypted BYTEA` (lịch sử các `AMK` cũ đã được wrap bởi `AMK` hiện tại, `NULL` nếu `key_version = 1`).
  - `key_version INT NOT NULL DEFAULT 1`.
- Bảng `sync_deltas`:
  - `crypto_version SMALLINT NOT NULL DEFAULT 1`.
  - `key_version INT NOT NULL DEFAULT 1`.

### 9.2. Endpoints Quản lý Khóa
- `POST /v1/auth/salt` — Trả về `kdf_salt` và `kdf_params` (có chống user enumeration).
- `GET /v1/keys/bundle` — Trả về `wrapped_amk`, `recovery_wrapped_amk`, `key_ring_encrypted`, `key_version`.
- `PUT /v1/auth/credentials` — Đổi mật khẩu nguyên tử (cập nhật `auth_verifier`, `kdf_salt`, `wrapped_amk` và revoke session khác).
- `PUT /v1/keys/rotate` — Xoay `AMK` (`key_version` tăng đơn điệu, có optimistic locking `expected_key_version`).

---

## 10. Kiểm thử & Tiêu chuẩn Nghiệm thu Bảo mật (Security Verification)

Mọi thay đổi trong `notes-crypto` phải vượt qua bộ kiểm thử bắt buộc trong CI:

1. **Test Vectors (KAT)**: Xác minh khớp 100% vector chuẩn RFC cho Argon2id, HKDF-SHA256 và XChaCha20-Poly1305.
2. **Tamper & AAD Binding Tests**:
   - Đảo 1 bit bất kỳ trong `ciphertext`, `nonce`, `auth_tag` → phải thất bại.
   - Giữ nguyên `encrypted_payload` nhưng đổi `user_id`, `entity_id`, `entity_type`, `client_op_id`, hoặc `key_version` trong AAD → phải trả về `CryptoError::AuthenticationFailed`.
3. **STREAM Blob Integrity Tests**:
   - Cắt bỏ chunk cuối (truncation), hoán đổi chunk 0 và chunk 1 (reordering), hoặc nhân đôi chunk → phải bị phát hiện và từ chối.
4. **Property-Based & Fuzz Testing**:
   - `proptest` cho round-trip encrypt/decrypt + padding với kích thước từ `0 B` đến `10 MiB`.
   - `cargo-fuzz` cho bộ parse Binary Envelope và Blob Header để đảm bảo không bao giờ panic/OOM khi nhận payload độc hại từ server.
5. **No-Secret-Leak Checks**:
   - Test đảm bảo `format!("{:?}", key)` không chứa byte nào của khóa.
   - Kiểm tra với note có nội dung, tag và ảnh được lưu trước khi khóa, sau khi khóa, trong phiên mở khóa, sau khi khóa lại và sau khi mở lại app. Xác minh bản chiếu cho phép, FTS, cache, metadata attachment và dọn blob mồ côi.
