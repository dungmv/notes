# M1.2 · CRDT và lưu editor bền vững

**Phụ thuộc** — [M0.2](./m0-2-rust-editor-ffi.md), [fixture M0](../../fixtures/m0/README.md) và [M1.1](./m1-1-local-storage-and-migrations.md).

## Phạm vi

Mở rộng editor Rust để mở trạng thái CRDT đã lưu của từng ghi chú, áp thao tác, undo/redo và trả snapshot hoặc patch. Chốt phiên bản dữ liệu và hợp đồng UTF-16. Lưu trạng thái tài liệu cùng tiêu đề, snippet, văn bản dẫn xuất và các cờ trong transaction. Cung cấp điểm tích hợp cho FTS và attachment dùng cùng transaction.

## Tiêu chí nghiệm thu

- [ ] Round-trip trạng thái CRDT qua SQLite giữ văn bản, kiểu đoạn, định dạng inline, ID và thứ tự block, checklist và bảng mẫu M0.
- [ ] Thao tác và undo/redo trả patch đúng revision và đúng range UTF-16 với tiếng Việt, emoji và range lỗi. Chạy hồi quy trên fixture M0.
- [ ] Commit theo đúng thứ tự và chỉ ACK sau transaction thành công. Lỗi giữa batch giữ nguyên dữ liệu và revision đã commit. Thử lại phần chưa ACK rồi mở phiên mới cho kết quả đúng một lần.
- [ ] Ghi hợp đồng phiên bản dữ liệu, lỗi, ACK và retry để Swift và C# dùng chung. Đo với tài liệu dài để đánh giá chi phí staging và ghi lại kết quả cho TD-009.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Hợp đồng lưu](../specs/02-data-model-and-storage.md#31-lưu-bền-vững-và-khôi-phục)
