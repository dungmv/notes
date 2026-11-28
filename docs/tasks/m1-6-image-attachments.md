# M1.6 · Dán ảnh và blob store

**Phụ thuộc** — [M1.1](./m1-1-local-storage-and-migrations.md), [M1.2](./m1-2-persistent-crdt-editor.md), [M1.4](./m1-4-macos-ffi-and-navigation.md) và [M1.5](./m1-5-macos-rich-text-and-checklists.md).

## Phạm vi

Tích hợp ảnh từ pasteboard macOS, chèn block ảnh vào vị trí caret và hiển thị qua adapter AppKit. Rust quản lý blob theo SHA-256 và gắn metadata với tham chiếu tài liệu trong transaction sau khi file đã được ghi bền vững.

## Tiêu chí nghiệm thu

- [ ] Dán ảnh giữa hai đoạn văn bản và mở lại thấy đúng ảnh tại đúng vị trí. Metadata trỏ tới hash đúng và ảnh mở được với mạng tắt.
- [ ] Dán cùng bytes vào hai ghi chú dùng chung blob. Thao tác trên một ghi chú giữ ảnh còn được ghi chú kia tham chiếu.
- [ ] Lỗi trước commit giữ tài liệu đã lưu đọc được. Sau retry, ảnh xuất hiện đúng một lần tại vị trí đã chọn. Ca gián đoạn sau khi ghi blob nhưng trước commit có kết quả phục hồi xác định.
- [ ] Có fixture ảnh, hash và kết quả kỳ vọng để dùng cho kiểm thử dừng tiến trình và chuyển dữ liệu sang M2.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Đặc tả blob store](../specs/02-data-model-and-storage.md#33-content-addressable-blob-store)
