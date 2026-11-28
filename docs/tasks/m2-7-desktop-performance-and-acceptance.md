# M2.7 · Đo hiệu năng và nghiệm thu desktop

**Phụ thuộc** — [M0.6](./m0-6-editor-prototype-acceptance.md) và M1 đạt nghiệm thu, [M2.5](./m2-5-desktop-fixtures-and-compatibility.md), [M2.6](./m2-6-desktop-durability-and-recovery.md).

## Phạm vi

Đo trên bản build ứng viên M2 của hai client, với fixture có phiên bản và kịch bản gồm tài liệu dài, gõ liên tiếp và IME. Tổng hợp kết quả native, tương thích, phục hồi và hiệu năng để quyết định đóng mốc.

## Tiêu chí nghiệm thu

- [ ] Đo cold start từ yêu cầu khởi chạy đến khi editor có focus và nhận ký tự đầu tiên, độ trễ từ input hoặc commit IME đến khi UI áp cập nhật, và thời gian từ thao tác hoàn tất đến ACK lưu. Ghi cách đặt mốc, cách chạy lại, số mẫu, p50/p95, cấu hình máy/ổ đĩa, OS, bản build và phiên bản fixture.
- [ ] Đối chiếu số đo có cùng định nghĩa với baseline trước đó, ghi sai khác về workload hoặc cấu hình và kết luận về chính sách commit trên tài liệu thực. Lưu kết quả để dùng làm baseline cho các bản tiếp theo.
- [ ] Biên bản ghi kết quả và bằng chứng từng ca trên cả hai OS, gồm FFI, tương thích hai chiều, offline và mở lại sau dừng cưỡng bức. Các ca bắt buộc đều đạt trước khi đóng M2.
- [ ] Cập nhật trạng thái nợ kỹ thuật liên quan theo bằng chứng. Mục đã xử lý có ngày hoàn thành, mục còn mở có giới hạn và hướng xử lý.

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Số đo M0](../m0-editor-prototype-results.md) · [Sổ nợ kỹ thuật](../technical-debt.md)
