# M4-W.3 · Core bảng đầy đủ, tag inline và Smart Folders

**Phụ thuộc** — Kết quả editor và tài liệu dùng chung từ M2/M3 (block Table cơ bản, hợp đồng `EditorOp`/patch). Chạy song song được với [M4-W.1](./m4-windows-1-locked-notes-core-and-sqlcipher.md).

## Phạm vi

- Hoàn thiện block `Table` trong `editor.rs`: thêm/xóa hàng và cột (kể cả vị trí giữa bảng), di chuyển hàng/cột, sửa và định dạng inline trong ô; hợp đồng `EditorOp` và patch cho từng thao tác qua `notes-ffi`; đo lại chi phí TOM/projection khi sửa bảng và ghi nhận giới hạn còn lại vào `TD-009` thay vì xử lý lặng lẽ.
- Tag inline: gõ `#từ-khóa` (Unicode/tiếng Việt) được nhận diện thành tag; `note_tags` cập nhật mỗi khi tài liệu đổi và khi xóa nội dung có tag theo [02 §1.3](../specs/02-data-model-and-storage.md#13-tag); phối hợp [M4-W.1](./m4-windows-1-locked-notes-core-and-sqlcipher.md) loại tag của ghi chú khi khóa.
- Smart Folders: DSL `smart_query` dạng JSON ([02 §1.1](../specs/02-data-model-and-storage.md#11-folder)) lọc theo tag, ngày sửa, `has:checklist`, attachment, trạng thái khóa; đánh giá query trên các cột dẫn xuất có sẵn, không đọc nội dung ghi chú khóa; tạo/đổi tên/xóa Smart Folder và `sort_order`.
- Thư mục con: `parent_id` nhiều cấp, di chuyển nguyên cụm, chống vòng lặp cha–con, cập nhật danh sách sidebar.
- Kiểm thử property/integration: CRDT hội tụ khi hai thiết bị đồng thời thêm/xóa hàng-cột và sửa cùng ô; tag nhất quán qua chuỗi sửa/xóa/undo; Smart Query trả đúng kết quả trên fixture dùng chung.

## Tiêu chí nghiệm thu

- [ ] Mọi thao tác bảng (thêm/xóa/di chuyển hàng-cột, sửa ô có định dạng) chạy qua đường lưu thật và hội tụ khi hai thiết bị sửa đồng thời ở các vùng khác nhau; ca sửa cùng ô ghi nhận kết quả hợp nhất rõ ràng.
- [ ] Gõ/xóa `#tag` cập nhật `note_tags` chính xác (kể cả tag tiếng Việt có dấu và emoji cạnh tag); FTS cột `tags` phản ánh theo.
- [ ] Smart Folder với điều kiện kết hợp (ví dụ `#work AND has:checklist`) trả đúng bộ ghi chú trên fixture; ghi chú khóa bị loại khỏi mọi Smart Query theo nội dung.
- [ ] Thư mục con tạo, đổi tên, di chuyển nguyên cụm và tránh vòng lặp; di chuyển ghi chú giữa cây thư mục giữ ghim và thứ tự.
- [ ] Số đo chi phí sửa bảng (độ trễ gõ trong ô, thời gian áp patch) ghi được và đối chiếu baseline M2; phần chưa đạt ghi vào `TD-009`.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Editor spec](../specs/03-features-and-editor.md#2-tính-năng-soạn-thảo-editor-capabilities) · [Mô hình dữ liệu](../specs/02-data-model-and-storage.md#1-thực-thể-cốt-lõi-domain-entities)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành: 6 thao tác cấu trúc bảng (thêm/xóa/di chuyển hàng-cột, undo/redo, ghi lại khóa vị trí CRDT sau đổi cấu trúc), InsertAt cho phép chèn bảng mới, tag inline → `note_tags` + cột `tags` trong FTS (schema v6), tìm `#tag`, Smart Folder DSL (`tag`/`has_checklist`/`has_attachments`/`folder`), thư mục con chống vòng lặp, FFI đầy đủ.
- Kiểm chứng: 7 test editor (bảng + history + hội tụ khóa vị trí), 8 test offline (tag/smart folder/khóa scrub tag), FFI 4 test.
- Còn lại: đo chi phí TOM khi sửa bảng ghi vào TD-009 (đã ghi nhận giới hạn render pill/table vào TD-018).
