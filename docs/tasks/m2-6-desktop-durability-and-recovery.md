# M2.6 · Lưu bền vững và phục hồi desktop

**Phụ thuộc** — Storage và migration M1, luồng Windows từ [M2.1](./m2-1-windows-build-and-offline-ffi.md) đến [M2.4](./m2-4-windows-attachments-and-links.md), fixture từ [M2.5](./m2-5-desktop-fixtures-and-compatibility.md).

## Phạm vi

Kiểm tra luồng ghi thật qua FFI trên cả hai OS với mạng tắt. Xác minh ACK, flush khi đổi vòng đời, lỗi ghi và thử lại, tính nhất quán của nội dung, chỉ mục tìm kiếm và blob, cùng migration từ phiên bản SQLite cũ được M1 hỗ trợ.

## Tiêu chí nghiệm thu

- [ ] Sau khi app báo **Đã lưu** cho lần sửa văn bản và dán ảnh, dừng cưỡng bức tiến trình ngay sau ACK. Mở lại và đối chiếu nội dung, định dạng, trạng thái checklist, ảnh và kết quả tìm kiếm đã commit.
- [ ] Khi ghi thất bại trước commit, lần sửa chưa ACK không được báo **Đã lưu**. Sau rollback, thử lại trong phiên commit đúng thứ tự, ảnh xuất hiện một lần tại vị trí mong đợi và chỉ mục khớp nội dung.
- [ ] Chuyển ghi chú hoặc đóng app bình thường yêu cầu flush. Nếu flush lỗi, UI báo trạng thái lưu theo hợp đồng M1 và giữ phần chờ lưu để thử lại trong phiên.
- [ ] Migration chạy trên bản sao SQLite cũ cùng blob tương ứng. Sau khi mở và ghi lại, ghi chú, liên kết, FTS và ảnh còn nhất quán. Lưu kết quả từng ca trên cả hai OS và cập nhật TD-010.

Phần Windows đã chạy hai ca dừng WinUI sau ACK, trigger lỗi SQLite cho văn bản và ảnh, thử lại, guard khi rời editor và migration schema 1 có blob. Giữ các mục nghiệm thu hai OS mở cho đến khi có lượt macOS. [Biên bản M2 Windows](../m2-windows-acceptance.md)

[Kế hoạch M2](../plans/20261128095337-m2-desktop-offline.md) · [Hợp đồng lưu và phục hồi](../specs/02-data-model-and-storage.md#31-lưu-bền-vững-và-khôi-phục) · [Sổ nợ kỹ thuật](../technical-debt.md)
