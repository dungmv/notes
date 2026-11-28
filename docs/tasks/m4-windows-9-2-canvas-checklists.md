# M4.W.9.2 · Checklist trong canvas

**Phụ thuộc** — [M4-W.9.1](./m4-windows-9-1-canvas-projection-mapping.md): checklist chỉ đạt trải nghiệm Apple Notes khi các mục nằm chung canvas với đoạn thường; [TD-013](../technical-debt.md) ghi nhận thứ tự trình bày hiện tại cần đối chiếu lại khi chuyển canvas.

## Phạm vi

- Mục checklist là một dòng trong canvas: glyph checkbox cục bộ đầu dòng (☐/☑), **không thuộc nội dung CRDT**; ánh xạ offset thêm/trừ chiều dài prefix theo dòng checklist.
- Bấm/touch vào glyph → `toggle` đúng mục trong canvas có nhiều checklist xen kẽ đoạn thường; hit-test qua vị trí ký tự được bấm.
- Enter cuối mục checklist tạo mục mới cùng cấp (`split_checklist`); Shift+Enter xuống dòng mềm trong cùng mục; Tab/Shift+Tab đổi cấp (`indent`) thể hiện bằng thụt lề dòng trong TOM.
- Gõ `[] ` ở đầu dòng paragraph chuyển dòng đó thành checklist (mở rộng trigger hiện tại theo block sang theo dòng).
- Tự sắp checklist khi bật: nhóm chưa hoàn tất trước, hoàn tất sau — trình bày trong cùng TOM (TD-013), thứ tự gốc giữ trong tài liệu; caret giữ đúng mục đang sửa khi sắp xếp.
- Định dạng inline (bold/italic/…) trên dòng checklist hoạt động cùng offset mapping.

## Tiêu chí nghiệm thu

- [ ] Bấm checkbox tick đúng mục trong canvas nhiều checklist xen kẽ đoạn thường; trạng thái giữ đúng sau khi mở lại.
- [ ] Enter tạo mục checklist tiếp theo, Shift+Return xuống dòng trong cùng mục, Tab/Shift+Tab đổi cấp theo [ca §0](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu).
- [ ] `[] ` tạo checklist tại dòng hiện tại mà không xóa nội dung sau trigger.
- [ ] Khi bật tự sắp: đánh dấu đưa mục xuống cuối nhóm, bỏ đánh dấu về nhóm chưa hoàn tất, không rớt định dạng và caret; thứ tự gốc vẫn nằm trong tài liệu (TD-013).
- [ ] IME tiếng Việt (Telex tích hợp) gõ đúng trên dòng checklist, kể cả khi prefix dịch offset.
- [ ] Undo/redo tick/bỏ tick/indent không lệch glyph hay nội dung.

[Kế hoạch M4-W.9](../plans/20261129012854-editor-single-canvas-windows.md) · [Spec §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) · [Sổ nợ kỹ thuật](../technical-debt.md)
