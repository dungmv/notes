# Technical Debt Register

Tài liệu theo dõi và kiểm soát toàn bộ nợ kỹ thuật (technical debt) trong dự án. Mọi giải pháp tạm thời, đánh đổi thiết kế, hoặc phần cần refactor đều phải được ghi vào đây theo quy định tại [AGENTS.md](../AGENTS.md).

---

## Active Technical Debt

| ID | Khu vực / File | Mô tả & Lý do | Mức độ | Hướng xử lý đề xuất | Trạng thái | Ngày ghi nhận |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| TD-001 | Editor, FFI và SQLite ([01](./specs/01-architecture-and-ffi.md), [02](./specs/02-data-model-and-storage.md), [03](./specs/03-features-and-editor.md)) | Adapter Windows và chi phí commit theo batch chưa được đo trên tài liệu thật. Ưu tiên độ đúng và độ bền trước khi tối ưu. | High | Chạy prototype chung trên macOS/Windows và đo độ trễ gõ, patch và commit. Chốt adapter và cách gom batch theo kết quả. | Open | 2026-11-27 |
| TD-002 | Sync backend ([06](./specs/06-backend-spring-boot.md)) | Ghi delta tuần tự theo tài khoản để bảo đảm cursor. Log Phase 3 tăng dần vì giữ lịch sử cho retry và thiết bị offline lâu. | Medium | Đo tranh chấp khóa và chi phí bootstrap. Thiết kế compaction có hợp đồng snapshot, phục hồi và giữ biên nhận. | Open | 2026-11-27 |
| TD-003 | Locked Notes ([02](./specs/02-data-model-and-storage.md), [07](./specs/07-e2ee-cryptography.md)) | Đồng bộ Locked Notes hoãn vì bản ghi và blob cũ có thể vẫn mở được bằng khóa của ghi chú thường. | High | Kiểm thử thay khóa, chuyển dữ liệu cục bộ cũ, xử lý log/snapshot trước khi khóa và đồng bộ với thiết bị offline. | Open | 2026-11-27 |
| TD-004 | Sync transport ([04](./specs/04-sync-and-security.md), [06](./specs/06-backend-spring-boot.md)) | Client polling qua HTTPS giúp giảm hạ tầng. Độ trễ nhận bản sửa phụ thuộc chu kỳ poll và có request ngay cả khi ít thay đổi. | Low | Đo số lượt pull rỗng và độ trễ nhận thay đổi khi cả hai máy đang mở hoặc vừa quay lại app. Điều chỉnh chu kỳ theo kết quả. | Open | 2026-11-27 |
| TD-005 | Khôi phục E2EE ([07](./specs/07-e2ee-cryptography.md)) | Đã mô tả lớp wrap phục hồi nhưng chưa có hợp đồng lấy bundle và đổi thông tin đăng nhập khi mất mật khẩu. | High | Đặc tả và kiểm thử luồng xác thực phục hồi, cập nhật nguyên tử thông tin đăng nhập và thu hồi phiên trước khi nghiệm thu Phase 3. | Open | 2026-11-27 |

---

## Resolved Technical Debt

| ID | Khu vực / File | Mô tả | Cách đã xử lý | Ngày hoàn thành |
| :--- | :--- | :--- | :--- | :--- |
| — | — | — | — | — |
