# M2.5 · Fixture chung và tương thích hai chiều

**Phụ thuộc** — [Fixture M0](../../fixtures/m0/README.md) để chuẩn bị. Chạy đủ sau [M0.6](./m0-6-editor-prototype-acceptance.md), M1 và [M2.1](./m2-1-windows-build-and-offline-ffi.md), [M2.2](./m2-2-windows-navigation-and-search.md), [M2.3](./m2-3-windows-editor.md), [M2.4](./m2-4-windows-attachments-and-links.md).

## Phạm vi

Phiên bản hóa fixture desktop có kết quả kỳ vọng, gồm dữ liệu thư mục/ghi chú, tiếng Việt, emoji, vùng chọn, rich text, checklist, bảng mẫu, ảnh và liên kết. Dùng cùng dữ liệu và kết quả kỳ vọng cho kiểm thử hợp đồng FFI và ca nghiệm thu native. Ghi thao tác riêng cho phím tắt và điều hướng mỗi OS.

## Tiêu chí nghiệm thu

- [ ] Fixture, lệnh chuẩn bị và ma trận ca có thể chạy lại từ trạng thái dữ liệu sạch. Ma trận bao phủ ca editor M0 và luồng M1, gồm thư mục, xóa/khôi phục, tìm kiếm tiếng Việt, phím tắt và focus.
- [ ] Kiểm thử Swift và C# gọi vào Rust thật. Cả hai kiểm tra UTF-16 và range lỗi, round-trip tài liệu, commit/ACK, callback bất đồng bộ và vòng đời tài nguyên qua các lượt mở/đóng lặp lại.
- [ ] Từ bản sao cùng fixture, A sửa và lưu offline, đóng phiên rồi chuyển dữ liệu bằng quy trình fixture nhất quán, gồm cả blob. B mở, kiểm tra, sửa và lưu. A đọc lại kết quả. Chạy quy trình với cả macOS và Windows ở vai trò A.
- [ ] Đối chiếu nội dung, định dạng, checklist, bảng mẫu, vị trí và hash ảnh, ID đích và kiểu nhãn liên kết sau mỗi lần mở. Lưu log và kết quả cho từng chiều cùng phiên bản fixture.

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Chiến lược kiểm thử](../specs/05-platform-clients-and-roadmap.md#4-chiến-lược-kiểm-thử-testing-strategy)
