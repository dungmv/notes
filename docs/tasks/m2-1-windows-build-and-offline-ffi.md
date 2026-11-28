# M2.1 · Build Windows và FFI offline

**Phụ thuộc** — [M0.4](./m0-4-windows-editor-prototype.md), [task tạo dự án WinUI](./create-winui-project.md) và API offline của [M1](../roadmap.md#m1--mvp-macos-offline). Có thể chuẩn bị build trước khi API M1 ổn định.

## Phạm vi

Mở rộng dự án WinUI và script build Rust DLL hiện có. Dựa trên C ABI đã chạy ở M0, đánh giá generator tương thích theo TD-008 và ghi quyết định binding theo kết quả. Nối phiên offline, ghi chú/thư mục, truy vấn tìm kiếm, editor và attachment của Rust core qua lớp gọi C# có quy định rõ quyền sở hữu handle và buffer, luồng callback và ánh xạ lỗi.

## Tiêu chí nghiệm thu

- [ ] Ghim phiên bản gói WinUI đã kiểm chứng, build và mở app từ môi trường Windows sạch trên các kiến trúc công bố hỗ trợ cho M2. Script đưa đúng DLL vào đầu ra build và tài liệu ghi lệnh chạy lại.
- [x] App mở và đóng phiên Rust với thư mục dữ liệu Windows. Tạo, đọc, cập nhật và tìm một ghi chú qua FFI với mạng tắt.
- [ ] Lỗi mở phiên hoặc thao tác lưu đi qua FFI và được trả về ứng dụng. Công việc bất đồng bộ giữ UI phản hồi, callback cập nhật UI trên dispatcher và được hủy đăng ký khi đóng phiên.
- [x] Có bằng chứng cho lựa chọn binding và cập nhật TD-007, TD-008 theo kết quả build và kiểm thử vòng đời.

M1 đã chạy x64 trên Windows Sandbox sạch. M2 dùng `LibraryImport` với marshalling UTF-8 do .NET sinh, giữ giao thức JSON của M1 và chuyển bước mở phiên sang worker. Kết quả mở được xử lý trên UI dispatcher. Lượt C# M2 mở/đóng tám lần trên fixture qua DLL thật. [Biên bản M2 Windows](../m2-windows-acceptance.md)

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Hợp đồng FFI](../specs/01-architecture-and-ffi.md) · [Sổ nợ kỹ thuật](../technical-debt.md)
