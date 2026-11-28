# M1.3 · Tìm kiếm tiếng Việt trong Rust core

**Phụ thuộc** — [M1.1](./m1-1-local-storage-and-migrations.md) và bản chiếu văn bản từ [M1.2](./m1-2-persistent-crdt-editor.md).

## Phạm vi

Tạo chỉ mục FTS5 cho tiêu đề và nội dung ghi chú thường. Dùng cùng quy tắc chuẩn hóa truy vấn và nội dung, hỗ trợ có dấu, không dấu và `đ`/`d`. Cung cấp truy vấn tìm toàn bộ cho FFI và quy tắc đối sánh trong ghi chú để adapter chọn đúng range văn bản gốc.

## Tiêu chí nghiệm thu

- [ ] Fixture tìm được ghi chú qua tiêu đề và nội dung với truy vấn có dấu, không dấu, khác cách mã hóa Unicode và `đ`/`d` theo quy tắc đã ghi.
- [ ] Kết quả và snippet trỏ đúng ghi chú. Tìm trong ghi chú trả vị trí ứng với chuỗi gốc, kể cả khi trước kết quả có emoji hoặc dấu tổ hợp.
- [ ] Tạo, sửa, xóa mềm, khôi phục và migration đưa chỉ mục về đúng dữ liệu đã commit. Lỗi ghi giữ kết quả tương ứng bản đã commit.
- [ ] Truy vấn rỗng, ký tự đặc biệt và chuỗi người dùng nhập có hành vi xác định. API chạy trên DB cục bộ khi tắt mạng.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Đặc tả FTS5](../specs/02-data-model-and-storage.md#32-full-text-search-fts5)
