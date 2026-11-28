# M3-W.5 · Đồng bộ blob mã hóa và kiểm thử bền vững trên Windows

**Phụ thuộc** — [M3-W.3](./m3-windows-3-sync-engine-and-ui-triggers.md), [M3-W.4](./m3-windows-4-live-merge-and-conflict-ux.md) và backend MinIO/S3 presigned endpoints.

## Phạm vi

Kiểm chứng toàn bộ luồng đồng bộ file đính kèm (ảnh) được mã hóa theo **STREAM-XChaCha20-Poly1305** (chunk 64 KiB) và các kịch bản lỗi/dừng tiến trình trên ứng dụng WinUI 3:
- Dán ảnh trên Windows → lưu blob cục bộ → mã hóa STREAM & tính `blob_id` (keyed HMAC) → `presign-upload` → PUT trực tiếp lên MinIO/S3 → `complete-upload` → chỉ push delta tham chiếu blob sau khi `complete-upload` thành công.
- Tải ảnh từ delta của thiết bị khác: `presign-download` → tải ciphertext → xác thực từng chunk & tag → giải mã STREAM → lưu blob cục bộ và hiển thị đúng vị trí trong editor WinUI.
- Kiểm thử bền vững trên tiến trình WinUI 3: dừng tiến trình đột ngột ngay sau ACK cục bộ (khi chưa kịp push), dừng giữa lúc đang upload blob/push/pull, ngắt mạng giữa chừng, retry sau mất phản hồi HTTP (`client_op_id` idempotency) và pull phân trang nhiều trang (`limit` nhỏ).

## Tiêu chí nghiệm thu

- [ ] Ảnh dán trên Windows được upload lên MinIO/S3 dưới dạng STREAM ciphertext và tải về hiển thị đúng nội dung, kích thước, vị trí trên thiết bị thứ hai (và ngược lại).
- [ ] Cùng một ảnh dán vào hai ghi chú trong cùng tài khoản được deduplicate trên server (`exists: true`), còn hai tài khoản khác nhau có `blob_id` và ciphertext khác nhau.
- [ ] Không có delta nào tham chiếu tới blob được push trước khi `complete-upload` xác nhận thành công.
- [ ] Dừng cưỡng bức tiến trình WinUI ngay sau ACK cục bộ hoặc giữa lượt push/pull rồi mở lại: không mất ghi chú/ảnh, không nhân bản delta nhờ `client_op_id`, cursor `last_server_cursor` chỉ tiến khi cả trang pull đã commit thành công.
- [ ] Pull phân trang nhiều trang (ví dụ 200+ delta với `limit` nhỏ) đồng bộ đầy đủ và rollback sạch nếu một trang bị lỗi giữa chừng.

[Kế hoạch M3 Windows](../plans/20261128144800-m3-windows-sync.md) · [STREAM Blobs](../specs/07-e2ee-cryptography.md#5-mã-hóa-file-đính-kèm-encrypted-blobs--safe-deduplication) · [Blob API](../specs/06-backend-spring-boot.md#44-blob-upload--download-s3presigner)
