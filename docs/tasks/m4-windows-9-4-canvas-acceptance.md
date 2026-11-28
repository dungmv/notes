# M4.W.9.4 · Nghiệm thu canvas và cập nhật nợ

**Phụ thuộc** — [M4-W.9.1](./m4-windows-9-1-canvas-projection-mapping.md), [M4-W.9.2](./m4-windows-9-2-canvas-checklists.md), [M4-W.9.3](./m4-windows-9-3-canvas-cross-block-editing.md). Kết quả ghép vào biên bản [M4-W.8](./m4-windows-8-desktop-acceptance.md) — canvas phải đạt trước khi nghiệm thu Desktop 1.0 phần Windows.

## Phạm vi

- Chạy lại toàn bộ ca editor [spec §0](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu) và ca mới [§2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) trên bản cài MSIX self-contained: tạo/sửa/mở lại, checklist, canvas liền mạch, IME, undo/redo, chuyển note giữ vùng chọn, lưu bền sau kill.
- Bộ gõ tiếng Việt: Vietnamese Telex tích hợp và UniKey phiên bản đã xác chứng (theo phạm vi ghi trong `TD-011`) commit/hủy đúng trên canvas một control.
- Hiệu năng: đo edit-to-projection và patch trên tài liệu dài (fixture 130.000 byte của M2), so sánh baseline M2; ghi kết quả vào `TD-009`.
- Trợ năng: Narrator đọc canvas như một vùng soạn thảo duy nhất (không đọc "nhiều input"); phím tắt và focus ở luồng chính đúng trên bản cài.
- Luồng tính năng khác không hồi quy: Locked Notes (màn hình khóa, auto-lock), Gallery, Quick Note, import/export, đồng bộ hai thiết bị.
- Cập nhật sổ nợ: `TD-009` (projection canvas thay control per-block, còn staging toàn tài liệu), `TD-011`, `TD-013` (thứ tự checklist trình bày sau khi chuyển canvas), đóng `TD-020` khi canvas đạt.

## Tiêu chí nghiệm thu

- [ ] Toàn bộ ca §0 và §2.3 đạt trên bản cài MSIX self-contained; biên bản ghi cấu hình máy và phiên bản bộ gõ.
- [ ] UniKey và Telex tích hợp: commit, hủy và composition với patch cùng control đạt trên canvas.
- [ ] Số đo hiệu năng ghi vào biên bản, không tệ hơn baseline M2 ở độ trễ gõ và patch.
- [ ] Narrator công bố canvas đúng vai trò soạn thảo; luồng khóa ghi chú và các luồng tính năng khác không hồi quy.
- [ ] Sổ nợ `TD-009`, `TD-011`, `TD-013`, `TD-020` cập nhật trạng thái kèm ngày; `TD-020` chuyển Resolved.
- [ ] Kết quả được trích dẫn trong biên bản M4-W.8.

[Kế hoạch M4-W.9](../plans/20261129012854-editor-single-canvas-windows.md) · [Spec §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) · [Sổ nợ kỹ thuật](../technical-debt.md)
