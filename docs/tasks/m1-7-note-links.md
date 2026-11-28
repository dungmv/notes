# M1.7 · Liên kết ghi chú

**Phụ thuộc** — [M1.2](./m1-2-persistent-crdt-editor.md), [M1.3](./m1-3-vietnamese-search.md), [M1.4](./m1-4-macos-ffi-and-navigation.md) và [M1.5](./m1-5-macos-rich-text-and-checklists.md).

## Phạm vi

Lưu link nội bộ bằng ID ghi chú đích và kiểu nhãn trong tài liệu Rust. Gắn gợi ý `>>`, lệnh chèn link và điều hướng ở editor. Lấy tiêu đề hiện tại của đích từ core khi nhãn dùng tiêu đề.

## Tiêu chí nghiệm thu

- [ ] Gõ `>>`, lọc và chọn ghi chú đích bằng bàn phím, chèn tại caret rồi mở đúng ID. `Cmd+K` gọi luồng chèn liên kết theo quy ước macOS.
- [ ] Đổi tên hoặc di chuyển ghi chú đích giữ nguyên đích. Nhãn theo tiêu đề cập nhật khi mở hoặc làm mới ghi chú, nhãn tùy chỉnh giữ nguyên.
- [ ] Mở lại và kiểm tra cả hai loại nhãn khi tắt mạng. Core trả trạng thái phù hợp khi đích bị xóa và mở lại được sau khi đích được khôi phục.
- [ ] Fixture xác định ID đích, vị trí và loại nhãn kỳ vọng để M2 kiểm tra vòng đọc–sửa hai chiều.

[Kế hoạch M1](../plans/20261128101843-m1-macos-offline.md) · [Ca nghiệm thu editor](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu)
