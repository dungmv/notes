# M2.3 · Editor rich text trên WinUI

**Phụ thuộc** — [M2.1](./m2-1-windows-build-and-offline-ffi.md), [M2.2](./m2-2-windows-navigation-and-search.md) cho luồng chuyển ghi chú, định dạng tài liệu và tập thao tác editor của M1.

## Phạm vi

Đưa adapter `RichEditBox` vào luồng editor offline. Dùng thao tác và patch Rust cho rich text, vùng chọn, undo/redo và checklist. Đánh giá bảng mẫu và undo khi patch chạm cùng block theo TD-009. Kiểm tra lại Vietnamese Telex và tái hiện ca UniKey đã ghi ở TD-011.

## Tiêu chí nghiệm thu

- [ ] Các kiểu đoạn và định dạng inline cơ bản của M1 chỉnh sửa được bằng toolbar hoặc phím tắt Windows tương ứng. Nội dung và định dạng mở lại đúng qua đường lưu thật.
- [ ] IME tiếng Việt commit và hủy đúng khi có patch, kể cả patch cùng control. Vị trí caret và vùng chọn đúng khi sửa cạnh emoji, undo/redo và chuyển ghi chú. Kết quả UniKey và phạm vi bộ gõ đã kiểm chứng được ghi vào biên bản.
- [ ] Checklist đạt các ca Return, Shift+Return, Tab/Shift+Tab, tick/bỏ tick và tự sắp xếp khi bật. Trạng thái, cấp và thứ tự giữ đúng sau khi mở lại.
- [ ] Mở và sửa một ô bảng fixture M0, giữ nội dung và định dạng các ô còn lại. Ghi kết quả thử patch chạm cùng block, undo/redo và tài liệu dài vào các mục nợ kỹ thuật liên quan.

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Ca editor](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu) · [Sổ nợ kỹ thuật](../technical-debt.md)
