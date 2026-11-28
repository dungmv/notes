# Editor canvas liền mạch — Windows (M4-W.9)

## Mục tiêu

Editor WinUI hiện dựng một `RichEditBox` cho từng block — mỗi đoạn, mỗi mục checklist, mỗi ô bảng là một ô nhập rời trong `StackPanel` ([`MainPage.xaml.cs`](../../clients/windows/Notes/MainPage.xaml.cs)). Một ghi chú vì thế hiển thị thành nhiều ô nhập có viền và khoảng cách: không chọn/xóa/định dạng được xuyên đoạn, con trỏ và IME chịu ranh giới control — khác hẳn chuẩn Apple Notes. Kế hoạch này chuyển lớp projection sang **canvas liền mạch** theo [spec editor §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing): chuỗi đoạn và checklist nằm trong một `RichEditBox` duy nhất, bảng/ảnh/thẻ liên kết là block nhúng tách run, không viền input, chọn và định dạng xuyên đoạn.

[Spec editor §2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing) · [Kết quả chốt Windows: canvas phân khúc](../specs/03-features-and-editor.md#kết-quả-chốt-windows-canvas-phân-khúc-m4-w9) · [Kế hoạch M4](./20261128183931-m4-desktop-1.0-windows.md)

## Điểm xuất phát và phụ thuộc

- **Từ M4 Windows**: W.1–W.7 đã thực thi (Locked Notes, bảng/tag/Smart Folders, import/export, MSIX, backup backend); M4-W.8 còn và phải chờ canvas đạt trước khi lập biên bản Desktop 1.0.
- **Không đổi hợp đồng dữ liệu**: giữ nguyên mô hình block và hợp đồng thao tác CRDT (`replace`, `split_paragraph`, `split_checklist`, `merge_previous`, `indent`, `toggle`, `paragraph_style`). Chỉ thay lớp projection trong `MainPage` — `views`/`Field` per-block thành projection canvas; `notes-crdt`, `notes-ffi` và `OfflineSession` không đổi, chỉ bổ sung helper ánh xạ offset nếu cần.
- **Nợ liên quan trực tiếp**: `TD-009` (staging toàn tài liệu, control riêng từng block), `TD-011` (UniKey không phát composition), `TD-013` (liên kết block tại caret, thứ tự checklist trình bày), `TD-020` (nợ UX "một ghi chú nhiều ô nhập" — mục tiêu chính của kế hoạch này).
- **Prototype M0**: chế độ `--m0-replay`, fixture replay và phím patch F6/F7/F8 dùng chung projection phải tiếp tục chạy PASS sau khi đổi.

[Sổ nợ kỹ thuật](../technical-debt.md)

## Trình tự triển khai

| Task | Kết quả | Phụ thuộc chính |
| --- | --- | --- |
| [M4-W.9.1 · Projection canvas và ánh xạ TOM ↔ block](../tasks/m4-windows-9-1-canvas-projection-mapping.md) | Một `RichEditBox` không viền cho chuỗi paragraph liên tiếp; style đoạn trong TOM; Enter/Shift+Enter/Backspace; ánh xạ offset tiếng Việt/emoji; fixture replay vẫn PASS | Editor M2.3 |
| [M4-W.9.2 · Checklist trong canvas](../tasks/m4-windows-9-2-canvas-checklists.md) | Checkbox glyph bấm được trong canvas, `[] ` tạo nhanh, Tab/Shift+Tab, Enter/Shift+Enter, tự sắp khi bật | M4-W.9.1 |
| [M4-W.9.3 · Chỉnh sửa xuyên khối và chuyển vùng](../tasks/m4-windows-9-3-canvas-cross-block-editing.md) | Chọn/xóa/định dạng nhiều đoạn, con trỏ đi qua bảng/ảnh/thẻ liên kết, reload sau sync giữ vùng chọn | M4-W.9.1 |
| [M4-W.9.4 · Nghiệm thu canvas và cập nhật nợ](../tasks/m4-windows-9-4-canvas-acceptance.md) | Toàn bộ ca editor đạt lại (IME, hiệu năng, fixture), sổ nợ cập nhật, ghép biên bản M4-W.8 | M4-W.9.1 – W.9.3 |

W.9.2 và W.9.3 chạy song song được sau khi W.9.1 ổn; W.9.4 ghép kết quả vào biên bản nghiệm thu [M4-W.8](../tasks/m4-windows-8-desktop-acceptance.md).

## Điều kiện đóng kế hoạch

- Một ghi chú văn bản + checklist hiển thị như một bề mặt liên tục: không khung nhập rời, không viền, giãn dòng đều — đối chiếu hành vi tham chiếu [§2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing).
- Chọn/xóa/định dạng xuyên đoạn và chuyển vùng qua block nhúng đạt ca W.9.3; Enter/Backspace/checklist đạt ca [§0](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu) và [§2.3](../specs/03-features-and-editor.md#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing).
- IME: Vietnamese Telex tích hợp và UniKey phiên bản đã xác chứng (`TD-011`) commit/hủy đúng trên canvas; emoji biên đoạn không lệch offset; fixture replay M0 và telemetry prototype vẫn PASS.
- Độ trễ gõ và patch không tệ hơn baseline M2 trên tài liệu dài; kết quả ghi vào `TD-009`.
- Sổ nợ `TD-009`, `TD-011`, `TD-013`, `TD-020` cập nhật trạng thái; kết quả ghép vào biên bản [M4-W.8](../tasks/m4-windows-8-desktop-acceptance.md) trước khi chốt Desktop 1.0 phần Windows.
