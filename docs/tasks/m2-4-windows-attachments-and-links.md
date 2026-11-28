# M2.4 · Dán ảnh và liên kết ghi chú trên Windows

**Phụ thuộc** — [M2.2](./m2-2-windows-navigation-and-search.md), [M2.3](./m2-3-windows-editor.md) và API attachment/liên kết của M1.

## Phạm vi

Đọc ảnh từ clipboard Windows, chèn ảnh vào tài liệu tại caret và lưu thông qua blob store của core. Gắn gợi ý `>>`, thao tác chèn liên kết và điều hướng tới ghi chú đích vào WinUI.

## Tiêu chí nghiệm thu

- [ ] Dán ảnh giữa hai phần văn bản, mở lại ghi chú và thấy đúng ảnh tại đúng vị trí. Core ghi bền vững blob trước khi commit tham chiếu. Lỗi lưu ảnh hiển thị được và có thể thử lại trong phiên.
- [ ] Gõ `>>`, dùng bàn phím chọn ghi chú đích và mở liên kết đến đúng ID. Phím `Ctrl+K` mở thao tác chèn liên kết theo quy ước Windows.
- [ ] Đổi tên ghi chú đích cập nhật nhãn lấy theo tiêu đề. Nhãn tùy chỉnh giữ nguyên sau đổi tên và mở lại.
- [ ] Ảnh và hai dạng nhãn đều sử dụng được với mạng tắt.

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Đặc tả editor](../specs/03-features-and-editor.md) · [Hợp đồng lưu](../specs/02-data-model-and-storage.md#31-lưu-bền-vững-và-khôi-phục)
