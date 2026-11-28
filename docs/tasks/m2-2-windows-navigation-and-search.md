# M2.2 · Điều hướng, tổ chức và tìm kiếm trên Windows

**Phụ thuộc** — [M2.1](./m2-1-windows-build-and-offline-ffi.md) và tập hành vi offline đã chốt ở M1.

## Phạm vi

Dựng sidebar thư mục, danh sách ghi chú và vùng editor trong WinUI. Nối các thao tác thư mục và vòng đời ghi chú theo M1 qua Rust core. Tìm toàn bộ qua FTS5 của core, tích hợp tìm trong ghi chú đang mở và điều hướng theo quy ước Windows.

## Tiêu chí nghiệm thu

- [ ] Người dùng dùng được cả chuột và bàn phím để chọn thư mục, chọn ghi chú, tạo, mở lại, di chuyển, xóa và khôi phục ghi chú. Danh sách phản ánh đúng thay đổi sau khi mở lại app.
- [x] `Ctrl+N` tạo ghi chú và đưa focus vào editor. `Ctrl+F` và `Ctrl+Shift+F` đưa focus đến tìm trong ghi chú và tìm toàn bộ tương ứng.
- [ ] Truy vấn có dấu và không dấu theo fixture M1 trả về ghi chú mong đợi. Sửa, xóa và khôi phục cập nhật kết quả tìm sau commit. Chọn kết quả mở đúng ghi chú.
- [ ] Các luồng trên chạy với mạng tắt.

M1 Windows xác nhận các phím tắt và FTS không dấu khi tắt mạng. M2 xác nhận query dấu tổ hợp sau mở lại, FTS cập nhật theo commit và khôi phục vùng chọn khi quay lại ghi chú. [Biên bản M2 Windows](../m2-windows-acceptance.md)

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Đặc tả client Windows](../specs/05-platform-clients-and-roadmap.md#2-windows-client-winui-3--windows-app-sdk)
