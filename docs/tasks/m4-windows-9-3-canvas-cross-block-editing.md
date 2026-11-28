# M4.W.9.3 · Chỉnh sửa xuyên khối và chuyển vùng

**Phụ thuộc** — [M4-W.9.1](./m4-windows-9-1-canvas-projection-mapping.md). Bảng/ảnh/thẻ liên kết vẫn là control nhúng tách run, nên hành vi qua ranh giới phải được tổng hợp tay theo [spec §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) ("block nhúng trong dòng chảy").

## Phạm vi

- **Chọn nhiều đoạn trong canvas**: xóa, gõ đè và định dạng vùng chọn phủ nhiều block; phân rã thành chuỗi thao tác CRDT (`replace` per block kèm `split`/`merge`) giữ ý định; undo trả về đúng trạng thái trước.
- **Định dạng vùng nhiều đoạn**: Ctrl+B/I/U/S, kiểu đoạn, liên kết áp cho toàn bộ block mà vùng chọn phủ, kể cả block một phần.
- **Chuyển vùng qua block nhúng**: mũi tên lên/xuống và trái/phải đi qua bảng, ảnh, thẻ liên kết đến đoạn kề mà không dừng; kéo chọn từ trước vào sau block nhúng; Enter/Backspace tại ranh giới hoạt động đúng.
- **Reload sau sync**: khi nhận patch ngoài (hai thiết bị sửa chỗ khác nhau), chiếu lại canvas và giữ vùng chọn như cơ chế `ReloadCurrentNotePreservingSelection` hiện có.
- **IME xuyên rung động diff**: composition không bị phá khi projection đến từ sync trong lúc đang gõ; giữ đường `deferred` hiện có cho patch đợi composition.
- Tài liệu dài: cập nhật và scroll mượt khi chọn/vá vùng qua nhiều block (không chiếu lại toàn canvas nếu không cần).

## Tiêu chí nghiệm thu

- [ ] Chọn ba đoạn xen một checklist rồi xóa: nội dung gộp đúng, undo trả lại đúng ba đoạn với định dạng gốc.
- [ ] Ctrl+B trên vùng phủ nhiều đoạn áp marks đúng mọi block bị phủ; phần vượt ranh giới block không rò sang block kề.
- [ ] Phím mũi tên đi xuống qua bảng đến đoạn sau không dừng; kéo chọn từ đoạn trước bảng sang đoạn sau bảng được.
- [ ] Backspace đầu đoạn kề ảnh/thẻ liên kết gộp đúng với đoạn trước; Enter cuối đoạn kề tạo đoạn mới đúng chỗ.
- [ ] Hai thiết bị sửa chỗ khác nhau rồi đồng bộ: canvas chiếu đúng, vùng chọn giữ khi reload; composition IME không bị phá bởi patch đến.
- [ ] Emoji biên đoạn và offset dài (surrogate) không lệch khi chỉnh xuyên block.

[Kế hoạch M4-W.9](../plans/20261129012854-editor-single-canvas-windows.md) · [Spec §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) · [Sổ nợ kỹ thuật](../technical-debt.md)
