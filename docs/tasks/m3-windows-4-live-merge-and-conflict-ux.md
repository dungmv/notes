# M3-W.4 · Cập nhật giao diện trực tiếp và bảo toàn IME khi merge

**Phụ thuộc** — [M3-W.1](./m3-windows-1-sync-ffi-and-secure-storage.md), [M3-W.3](./m3-windows-3-sync-engine-and-ui-triggers.md) và [M2.3 · Editor WinUI](./m2-3-windows-editor.md).

## Phạm vi

Xử lý các sự kiện thay đổi dữ liệu từ quá trình pull (`NoteContentUpdated`, `FoldersChanged`, `NoteListInvalidated`) trên giao diện ba cột WinUI 3:
- Cập nhật danh sách thư mục, danh sách ghi chú (tiêu đề, snippet, ghim, thời gian cập nhật, thùng rác) và nhãn liên kết ghi chú khi có thay đổi từ thiết bị khác.
- Khi ghi chú đang mở nhận delta từ server, áp phần thay đổi vào `RichEditBox` / block checklist / bảng mà không reset toàn bộ buffer đang soạn, giữ nguyên vị trí con trỏ/vùng chọn hợp lý và tuyệt đối không ghi đè hoặc phá vỡ composition của bộ gõ tiếng Việt (Windows Telex tích hợp và UniKey).
- Xử lý các tình huống biên khi đang mở ghi chú: ghi chú bị xóa vào thùng rác hoặc di chuyển sang thư mục khác từ thiết bị kia, và giữ/phục hồi bản nháp cục bộ nếu có xung đột revision (`TD-014`).

## Tiêu chí nghiệm thu

- [ ] Khi thiết bị khác tạo, đổi tên, di chuyển thư mục, ghim, xóa hoặc khôi phục ghi chú, sidebar và danh sách ghi chú trên Windows tự cập nhật đúng sau lượt pull.
- [ ] Khi đang mở và gõ tiếng Việt (Telex) trên một ghi chú ở Windows đồng thời nhận delta chỉnh sửa ở đoạn khác từ thiết bị thứ hai, nội dung từ xa hiển thị đúng mà không làm mất chữ đang gõ dở hoặc mất focus `RichEditBox`.
- [ ] Các thao tác ngoài CRDT (di chuyển thư mục, ghim/bỏ ghim, xóa/khôi phục) áp dụng đúng quy tắc merge của core và hiển thị nhất quán trên UI Windows.
- [ ] Khi ghi chú đang mở bị xóa ở thiết bị khác, UI chuyển trạng thái hợp lý (hiển thị thông báo hoặc chuyển sang trạng thái thùng rác) mà không crash hay ghi đè dữ liệu sai.

[Kế hoạch M3 Windows](../plans/20261128144800-m3-windows-sync.md) · [Threading & Editor](../specs/01-architecture-and-ffi.md#4-threading--async-model) · [Luồng đồng bộ](../specs/04-sync-and-security.md#12-luồng-đồng-bộ-sync-loop)
