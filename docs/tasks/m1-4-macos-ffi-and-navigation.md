# M1.4 · UniFFI và giao diện macOS ba cột

**Phụ thuộc** — Dự án Xcode, NotesCore và script build hiện có. Tích hợp dữ liệu cần [M1.1](./m1-1-local-storage-and-migrations.md), [M1.2](./m1-2-persistent-crdt-editor.md) và [M1.3](./m1-3-vietnamese-search.md).

## Phạm vi

Mở rộng `notes-ffi` và Swift bindings cho phiên offline, truy vấn dữ liệu, editor và lỗi có kiểu. Dùng Application Support cho dữ liệu người dùng và thư mục riêng cho chạy thử. Gắn sidebar, danh sách và editor trong SwiftUI `NavigationSplitView` vào core.

## Tiêu chí nghiệm thu

- [ ] Từ checkout sạch trên macOS, tài liệu có lệnh tạo bindings, build XCFramework và mở app qua Xcode. App mở được DB cục bộ. Ghi kiến trúc và phiên bản Xcode/macOS đã xác minh theo TD-006.
- [ ] Swift gọi Rust thật qua UniFFI. I/O chạy bất đồng bộ, cập nhật giao diện qua `MainActor` và hủy đăng ký callback khi kết thúc phiên. Lỗi mở dữ liệu và lỗi thao tác hiện được trên UI.
- [ ] Sidebar, danh sách và editor cho phép dùng chuột hoặc bàn phím để tạo, mở lại, di chuyển, xóa và khôi phục ghi chú. Tiêu đề và snippet theo dữ liệu core sau commit.
- [ ] `Cmd+N` tạo ghi chú và focus editor. `Cmd+F` tìm trong ghi chú. `Cmd+Opt+F` tìm toàn bộ. Chọn kết quả mở đúng ghi chú hoặc vùng văn bản.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Kiến trúc FFI](../specs/01-architecture-and-ffi.md)
