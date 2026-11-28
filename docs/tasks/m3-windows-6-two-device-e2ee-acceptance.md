# M3-W.6 · Nghiệm thu đồng bộ hai thiết bị và bảo mật trên Windows

**Phụ thuộc** — [M3-W.2](./m3-windows-2-auth-recovery-and-device-ui.md) đến [M3-W.5](./m3-windows-5-encrypted-blobs-and-offline-durability.md), cùng một thiết bị thứ hai (macOS hoặc Windows thứ hai) kết nối chung backend self-host.

## Phạm vi

Thực hiện bộ ca nghiệm thu chính thức cho M3 trên nền tảng Windows theo [Roadmap M3](../roadmap.md#m3--beta-đồng-bộ-e2ee):
- Chạy kịch bản hai thiết bị sửa offline trên cùng tập ghi chú (tiêu đề, rich text, checklist, ảnh, thư mục, ghim, xóa/khôi phục) rồi đồng bộ qua backend self-host.
- Tách riêng kết quả kiểm tra **hội tụ** (hai máy giống hệt nhau sau sync) và **bảo toàn chỉnh sửa độc lập** ở các vùng/thuộc tính khác nhau.
- Đo thời gian end-to-end từ lúc thiết bị A xác nhận đã lưu đến lúc thiết bị B (Windows) hiển thị bản sửa trong hai trường hợp: B đang mở sẵn và B vừa quay lại app.
- Kiểm tra ranh giới tin cậy trên PostgreSQL, MinIO và log của Spring Boot/WinUI; hoàn tất kiểm tra các ca thêm thiết bị, đổi mật khẩu, phục hồi tài khoản (`TD-005`) và thu hồi thiết bị.
- Lập biên bản nghiệm thu M3 Windows và cập nhật [sổ nợ kỹ thuật](../technical-debt.md).

## Tiêu chí nghiệm thu

- [ ] Hai thiết bị hội tụ hoàn toàn sau khi sửa offline; ghi nhận riêng kết quả hội tụ và kết quả giữ các chỉnh sửa độc lập ở cả chiều Windows ↔ thiết bị thứ hai.
- [ ] Có bảng số đo độ trễ đồng bộ (số mẫu, p50/p95, chu kỳ polling, cấu hình máy, phiên bản Windows, bản build) cho cả hai tình huống: app đang mở và vừa quay lại app.
- [ ] Xác minh trực tiếp trên PostgreSQL, MinIO và log server/client: không có plaintext tiêu đề, nội dung, tên thư mục, tên file ảnh hay khóa bí mật (`AMK`, `KEK`, `MS`, passphrase) bị lộ.
- [ ] Toàn bộ ca vòng đời tài khoản/thiết bị (thêm thiết bị, đổi mật khẩu, phục hồi bằng Recovery Key, thu hồi thiết bị) đạt trên bản build Windows.
- [ ] Có biên bản nghiệm thu M3 Windows (liên kết log và môi trường chạy) và đã cập nhật trạng thái các mục liên quan trong `docs/technical-debt.md` (`TD-004`, `TD-005`, `TD-014`, v.v.).

[Kế hoạch M3 Windows](../plans/20261128144800-m3-windows-sync.md) · [Roadmap M3](../roadmap.md#m3--beta-đồng-bộ-e2ee) · [Kiểm thử E2EE](../specs/07-e2ee-cryptography.md#10-kiểm-thử--tiêu-chuẩn-nghiệm-thu-bảo-mật-security-verification)
