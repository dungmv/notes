# M1.8 · Nghiệm thu MVP macOS và bàn giao M2

**Phụ thuộc** — [M0.6](./m0-6-editor-prototype-acceptance.md), các task M1.1–M1.7 trong [kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) và máy macOS có Xcode cùng bộ gõ tiếng Việt.

## Phạm vi

Chạy bản build ứng viên trên macOS với mạng tắt. Tập hợp fixture có phiên bản, kiểm thử hợp đồng Swift gọi Rust thật và thực thi ma trận UI, lưu, phục hồi và migration. Bàn giao đường build, dữ liệu và hợp đồng dùng lại ở Windows.

## Tiêu chí nghiệm thu

- [ ] Toàn bộ luồng M1 đạt trên app native với DB mới và mạng tắt. Lưu bản build, môi trường, fixture, thao tác, kết quả và bằng chứng từng ca, gồm kiểm thử FFI và hồi quy editor M0.
- [ ] Dừng cưỡng bức tiến trình ngay sau ACK cho lượt sửa nội dung và lượt dán ảnh. Sau khi mở lại, đối chiếu nội dung, checklist, vị trí và hash ảnh cùng kết quả FTS. Chạy thêm flush khi đổi ghi chú/thoát, lỗi ghi và thử lại qua FFI, migration từ fixture DB cũ cùng blob.
- [ ] Đo độ trễ từ thao tác hoàn tất đến ACK qua app thật trên fixture và tài liệu dài. Ghi số mẫu, p50/p95, macOS, cấu hình máy, bản build và kết luận về chính sách commit. Cập nhật TD-001, TD-006, TD-009 và TD-010 theo kết quả có được.
- [ ] Bàn giao cho M2 API offline và lỗi FFI, giao thức lưu, phiên bản schema/tài liệu, bộ ca và fixture có thể tái tạo gồm SQLite, ảnh và kết quả kỳ vọng. Có quy trình chuẩn bị bản sao nhất quán của DB và blob để chạy kiểm thử hai chiều.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Sổ nợ kỹ thuật](../technical-debt.md)
