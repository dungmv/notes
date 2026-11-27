# M0.2 · Xây hợp đồng editor Rust và FFI

**Phụ thuộc** — [M0.1 · Tài liệu và ca thử chung](./m0-1-shared-fixtures.md).

Triển khai phần mô hình CRDT cần cho fixture và xuất hợp đồng editor từ Rust qua FFI. Quy định rõ offset UTF-16 ở biên FFI, revision và thứ tự cập nhật.

## Tiêu chí nghiệm thu

- [ ] Client mở được tài liệu mẫu, gửi batch thao tác và nhận patch gia tăng theo block.
- [ ] Test Rust bao phủ round-trip, range ngoài biên, cắt cặp surrogate và kết quả patch trên fixture.
- [ ] Swift gọi được API editor qua UniFFI và API phiên bản hiện có tiếp tục hoạt động.

[Kế hoạch M0](../plans/20261127174136-m0-editor-ffi-prototype.md)
