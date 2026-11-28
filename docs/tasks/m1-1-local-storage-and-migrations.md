# M1.1 · Dữ liệu cục bộ và migration SQLite

**Phụ thuộc** — Rust workspace hiện có và [đặc tả dữ liệu](../specs/02-data-model-and-storage.md).

## Phạm vi

Tạo lớp lưu trữ Rust dùng chung với thư mục dữ liệu truyền từ client. Quản lý phiên bản schema SQLite, dữ liệu thư mục và ghi chú, truy vấn danh sách, di chuyển, xóa mềm và khôi phục. Xác định cách trả lỗi khi vị trí khôi phục cũ đã mất và ghi hành vi vào hợp đồng M1.

## Tiêu chí nghiệm thu

- [ ] Mở thư mục dữ liệu trống tạo DB và thư mục hệ thống theo schema M1. Mở lại dùng cùng dữ liệu. Kết nối ghi áp dụng WAL, `synchronous=FULL` và kiểm tra khóa ngoại.
- [ ] Tạo, đổi tên và liệt kê thư mục. Tạo, mở và di chuyển ghi chú giữ ID ổn định. Xóa mềm chuyển ghi chú vào Recently Deleted và khôi phục đưa ghi chú về vị trí hợp lệ theo hợp đồng.
- [ ] Migration từ phiên bản cũ được hỗ trợ giữ ID, quan hệ thư mục và dữ liệu ghi chú. Có fixture đầu vào và kết quả kỳ vọng. Migration lỗi rollback và trả lỗi cho caller.
- [ ] Kiểm thử tích hợp Rust chạy các thao tác trên SQLite thật, gồm mở lại DB và kiểm tra nhất quán khi thao tác thất bại.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md)
