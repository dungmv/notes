# M4-W.7 · Diễn tập sao lưu/khôi phục backend và quyết định compaction

**Phụ thuộc** — Backend self-host M3 ổn định (Spring Boot + PostgreSQL + MinIO). Độc lập với các task client; chạy được song song từ đầu mốc.

## Phạm vi

- Runbook sao lưu/khôi phục trên docker-compose: PostgreSQL (pg_dump/base backup) và MinIO (mirror bucket), tần suất đề xuất, RPO/RTO quan sát được; kiểm tra nhất quán giữa DB và object storage (blob mồ côi hoặc thiếu sau restore).
- Khôi phục trên môi trường sạch: dựng lại stack từ bản sao lưu, client Windows đồng bộ tiếp — không nhân bản delta nhờ `client_op_id`, cursor/`last_sync_seq` nhất quán, blob upload lại đúng sau phục hồi; ghi chú tạo sau khi server chết trước khi restore được xử lý đúng.
- Số đo cho `TD-002`: tăng trưởng `sync_deltas`, chi phí bootstrap thiết bị mới và thiết bị offline lâu, tranh chấp khóa theo tài khoản, chi phí polling; tổng hợp từ bản self-host đang chạy.
- Quyết định compaction có bằng chứng: nếu làm — đặc tả hợp đồng snapshot theo [06 §4.3](../specs/06-backend-spring-boot.md#43-compaction) (chứng minh snapshot phủ các delta bị thay thế, thiết bị mới/offline lâu, giữ biên nhận cho retry, ghi đồng thời khi tạo snapshot) rồi triển khai và thử thiết bị offline lâu retry qua ranh giới snapshot khi có thay đổi; nếu không làm — ghi rõ số liệu dẫn đến việc hoãn và mốc đề xuất.
- Cập nhật [runbook vận hành](../specs/06-backend-spring-boot.md#5-vận-hành-bảo-mật--mở-rộng) và trạng thái `TD-002`.

## Tiêu chí nghiệm thu

- [ ] Khôi phục PostgreSQL + MinIO từ bản sao lưu thành công trên môi trường sạch theo runbook; thời gian khôi phục và kích thước backup ghi được.
- [ ] Client Windows kết nối lại sau phục hồi: pull tiếp từ cursor đúng, không mất và không nhân bản delta; ảnh upload lại đúng sau khi object storage được dựng từ backup.
- [ ] Số đo `TD-002` có bảng số liệu (kích thước log, thời gian bootstrap, độ trễ pull) và một quyết định ghi rõ: làm compaction trong M4 hoặc hoãn với mốc đề xuất.
- [ ] Nếu triển khai compaction: thiết bị offline lâu vượt ranh giới snapshot khi có thay đổi vẫn hội tụ đúng với thiết bị online; biên nhận retry vẫn hợp lệ.
- [ ] Runbook được review lại và ghi vào spec vận hành kèm ngày kiểm chứng.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Backend spec](../specs/06-backend-spring-boot.md#43-compaction) · [TD-002](../technical-debt.md)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành: pg_dump custom-format 40KB; restore drill vào DB test khớp users=3/sync_deltas=13/devices=7; backup MinIO (volume data 8 object metadata bucket notes-blobs); runbook ghi vào [06 §5](../specs/06-backend-spring-boot.md#5-vận-hành-bảo-mật--mở-rộng); số đo TD-002 và quyết định hoãn compaction có bằng chứng.
- Còn lại: khôi phục trên môi trường sạch hoàn toàn (dựng compose mới từ backup — còn phụ thuộc hạ tầng demo) và ca client đồng bộ tiếp sau phục hồi.
