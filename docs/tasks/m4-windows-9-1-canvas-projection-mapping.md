# M4.W.9.1 · Projection canvas và ánh xạ TOM ↔ block

**Phụ thuộc** — [M2.3 editor rich text WinUI](./m2-3-windows-editor.md) cho hợp đồng thao tác và patch; [kế hoạch canvas liền mạch](../plans/20261129012854-editor-single-canvas-windows.md). Hợp đồng CRDT (`replace`, `split_paragraph`, `merge_previous`, `paragraph_style`, …) giữ nguyên; chỉ thay lớp projection trong `MainPage`.

## Phạm vi

- Gom chuỗi paragraph **liên tiếp** thành một `RichEditBox` duy nhất: TOM paragraph ↔ block, bảng/ảnh/thẻ liên kết vẫn là control nhúng và tách run. Không viền, không khoảng cách rời giữa các đoạn (`Spacing = 0`, đoạn trống đúng một dòng).
- Style đoạn áp trong TOM bằng `CharacterFormat`/`ParagraphFormat` (Title, Heading, Subheading, Body, Quote, Monospaced); dòng đầu là Title như Apple Notes.
- Lớp diff mới: so TOM với nội dung block theo **đoạn**, sinh `replace`/`split_paragraph`/`merge_previous` đúng block; ánh xạ offset UTF-16 (tiếng Việt có dấu, emoji, surrogate pair) giữa TOM và hợp đồng block.
- Enter (cuối đoạn → `split_paragraph`), Shift+Enter (ngắt dòng mềm `\v` trong cùng block, không tách TOM paragraph), Backspace đầu đoạn (`merge_previous`) — thay các handler per-field hiện tại.
- IME: giữ đường TextComposition hiện có trên control canvas; kiểm tra composition không bị diff cục bộ chen ngang.
- Cập nhật đường prototype M0 (`--m0-replay`, fixture replay, F6/F7/F8) để dùng projection mới.

## Tiêu chí nghiệm thu

- [ ] Ghi chú nhiều đoạn hiển thị đúng **một** khung nhập liền mạch, không viền, giãn dòng đều; đoạn trống chiếm đúng một dòng.
- [ ] Enter tạo đoạn mới trong cùng canvas, Backspace ở đầu đoạn gộp với đoạn trước; caret đúng vị trí sau split/merge, kể cả cạnh emoji và tiếng Việt có dấu.
- [ ] Đổi kiểu đoạn (Title ↔ Body ↔ Quote…) qua toolbar/phím tắt không rời canvas, không nhảy caret.
- [ ] Shift+Enter xuống dòng mềm trong cùng block; mở lại nội dung và khôi phục đúng qua đường lưu thật.
- [ ] Fixture replay M0 và telemetry prototype (edit-to-projection, patch) PASS với projection mới.
- [ ] Undo/redo qua core giữ ý định trên canvas một control (không lệch vùng chọn khi undo).

[Kế hoạch M4-W.9](../plans/20261129012854-editor-single-canvas-windows.md) · [Spec §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) · [Sổ nợ kỹ thuật](../technical-debt.md)
