# M1.5 · Editor TextKit 2 và checklist

**Phụ thuộc** — [M0.3](./m0-3-macos-editor-prototype.md), [M1.2](./m1-2-persistent-crdt-editor.md) và [M1.4](./m1-4-macos-ffi-and-navigation.md).

## Phạm vi

Đưa adapter `NSTextView` vào luồng ghi chú thật. Áp patch gia tăng, giữ caret và vùng chọn, nối undo/redo, kiểu Title/Heading/Body, định dạng inline cơ bản và checklist. Hiển thị phần đang lưu, đã lưu và lỗi lưu theo ACK của ghi chú đang sửa.

## Tiêu chí nghiệm thu

- [ ] Xác nhận TextKit 2 hoạt động khi mở và sửa ghi chú. Gõ Telex, commit/hủy composition khi có patch, sửa cạnh emoji, undo/redo và chuyển ghi chú giữ nội dung và vùng chọn theo fixture.
- [ ] Kiểu đoạn, bold/italic/underline và checklist chỉnh sửa được bằng toolbar hoặc phím tắt tương ứng và giữ nguyên khi mở lại.
- [ ] Checklist đạt Return, Shift+Return, Tab/Shift+Tab và tick/bỏ tick. Khi bật tự sắp xếp, mục đã tick xuống cuối và mục bỏ tick trở về nhóm chưa hoàn tất.
- [ ] UI phản hồi khi gõ và chỉ báo **Đã lưu** khi nhận ACK cho phần thay đổi đang hiển thị. Đổi ghi chú hoặc thoát bình thường yêu cầu flush. Khi lỗi, app cho thử lại phần chưa ACK trong phiên và giữ đúng ghi chú đích.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Ca nghiệm thu editor](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu)
