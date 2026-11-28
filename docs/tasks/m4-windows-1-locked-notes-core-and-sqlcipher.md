# M4-W.1 · Locked Notes trong Rust core và SQLCipher

**Phụ thuộc** — `notes-crypto` từ M3 (STREAM AEAD, zeroize, các kiểu khóa theo [07 §7.3](../specs/07-e2ee-cryptography.md#73-vệ-sinh-bộ-nhớ-trong-rust-notes-crypto)). Không phụ thuộc task M4 khác; chạy song song được với [M4-W.3](./m4-windows-3-tables-tags-smart-folders-core.md).

## Phạm vi

- Thêm `NoteLockKey` (NLK) vào `notes-crypto`/`notes-app`: sinh khóa ngẫu nhiên, `lock_metadata` (salt, KDF params, wrapped note key, biometric hint) theo [02 §1.2](../specs/02-data-model-and-storage.md#12-note), mã hóa `crdt_state` bằng NLK khi khóa.
- Chuyển ghi chú thường → khóa theo [chính sách 02 §3.4](../specs/02-data-model-and-storage.md#34-chính-sách-lưu-trữ-locked-notes): ghi bản mã hóa nội dung và manifest attachment; trong cùng transaction xóa bản ghi FTS, tag, metadata attachment dạng rõ, snippet, cache văn bản và các cờ suy ra từ nội dung; blob khóa ghi bền vững **trước** commit; sau commit dọn bản rõ cục bộ khi không còn ghi chú thường nào tham chiếu. Gỡ bảo vệ khôi phục các bản ghi tìm kiếm và metadata.
- Phiên mở khóa chỉ dựng nội dung, attachment và dữ liệu dẫn xuất trong bộ nhớ; khóa lại giải phóng NLK, plaintext, cache UI và preview khỏi RAM (zeroize, không bản chiếu mới trên đĩa).
- SQLCipher cho SQLite kể cả WAL: khóa cục bộ sinh ngẫu nhiên, key material truyền từ client qua FFI; quy trình chuyển kho cũ chưa mã hóa (file DB, WAL, bản sao) sang kho mã hóa kèm migration schema trước khi bật tính năng.
- FFI: API khóa/mở khóa/đổi mật khẩu khóa/trạng thái khóa và event tương ứng; auto-lock khi hết N phút không tương tác hoặc nhận sự kiện khóa màn hình/sleep từ client; chặn khóa ghi chú trên thư viện đã bật sync và chặn bật sync khi thư viện có ghi chú khóa (`TD-003`).
- Kiểm thử Rust (unit/integration): ghi chú tạo trước khi khóa, khóa lại nhiều lần, khởi động lại giữa phiên mở khóa, dừng tiến trình giữa lúc khóa (blob mồ côi, retry), FTS rỗng cho nội dung khóa nhưng tiêu đề vẫn tìm được qua `notes.title`.

## Tiêu chí nghiệm thu

- [ ] Ghi chú đã tạo trước khi bật tính năng được chuyển sang khóa đúng chính sách; sau khóa: FTS/tag/snippet/thumbnail/metadata attachment dạng rõ đã xóa, tiêu đề, thư mục, ghim và mốc thời gian còn dùng được.
- [ ] Kill tiến trình ngay sau commit khóa rồi mở lại: bản mã và lock_metadata còn nguyên, không còn plaintext hoặc bản ghi FTS sót; mở khóa khôi phục đúng nội dung, checklist và ảnh.
- [ ] Kho SQLite (kể cả WAL) chuyển sang SQLCipher với dữ liệu M2/M3 hiện có; quy trình chuyển đổi xử lý được file DB/WAL chưa mã hóa; mở file bằng công cụ SQLite thường không đọc được nội dung.
- [ ] Đổi mật khẩu khóa tạo wrapped note key mới và re-wrap nội dung an toàn; nhập sai mật khẩu và ciphertext bị can thiệp đều bị từ chối; zeroize xác nhận trên FFI boundary.
- [ ] Auto-lock đúng sự kiện (Lock Now, hết hạn N phút, khóa màn hình/sleep): sau khóa lại, RAM không giữ NLK/plaintext và đĩa không có bản chiếu mới của nội dung khóa.
- [ ] Bật sync bị chặn khi thư viện có ghi chú khóa; khóa ghi chú bị chặn khi sync đã bật; `TD-003` giữ trạng thái Open kèm ghi nhận phạm vi đã làm ở phần cục bộ.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Lưu trữ](../specs/02-data-model-and-storage.md#34-chính-sách-lưu-trữ-locked-notes) · [E2EE §7](../specs/07-e2ee-cryptography.md#71-mã-hóa-cơ-sở-dữ-liệu-cục-bộ)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành: `notes-crypto::notelock` (NLK, wrap/unwrap Argon2id, AAD gắn note id), khóa/gỡ bảo vệ/đổi mật khẩu theo chính sách 02 §3.4, SQLCipher (kể cả WAL) + chuyển kho dạng rõ kèm migration v1→v6, chặn khóa khi sync bật và chặn bật sync khi có ghi chú khóa, zeroize qua `ZeroizeOnDrop`.
- Kiểm chứng: 9 test core (`offline::locked::tests`) + FFI end-to-end (`locked_notes_flow_through_ffi`): khóa→tìm→mở→image_data→sửa→khóa lại→khởi động lại→gỡ bảo vệ.
- Còn lại: kill-tiến-trình-ngay-sau-commit-khóa chạy trên bản build thật (harness M2 reopen đã có, chưa lặp cho ca khóa).
