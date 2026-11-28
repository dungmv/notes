# M4-W.4 · UI tổ chức: bảng, tag, Smart Folders, Gallery, Quick Note

**Phụ thuộc** — [M4-W.3](./m4-windows-3-tables-tags-smart-folders-core.md) (FFI bảng/tag/smart query/thư mục con).

## Phạm vi

- UI bảng trên `RichEditBox` trong giới hạn adapter đã chốt (`TD-009`): toolbar thao tác hàng/cột, điều hướng bằng phím giữa các ô, định dạng inline trong ô; phần RichEditBox chưa hỗ trợ được ghi rõ vào `TD-009`/`TD-014` kèm phương án.
- Tag Browser trong sidebar: danh sách `#tag` đang có, chọn nhiều tag để lọc giao AND, đếm số ghi chú theo tag; tag pill trong editor theo khả năng renderer.
- Smart Folders trong sidebar: tạo/sửa điều kiện bằng form đơn giản (tag, checklist, attachment, khoảng ngày), icon riêng, hiển thị số ghi chú; không cho di chuyển ghi chú vào Smart Folder.
- Gallery View: chuyển đổi List View ↔ Gallery View dạng thẻ có thumbnail; nhóm **Pinned** trên cùng và phân nhóm thời gian (Today, Previous 7 Days, Previous 30 Days, theo tháng) hoạt động ở cả hai chế độ; thumbnail lấy từ blob store, bỏ qua ghi chú khóa.
- Quick Note: cửa sổ nổi mở bằng phím tắt toàn cục `Win+Alt+N` kể cả khi app chạy nền (system tray), ghi nhanh vào thư mục Quick Notes, đóng cửa sổ vẫn lưu theo đường commit core.
- Filter chip trong Note List: `Locked`, `Checklists`, `Attachments`, `#tag` theo [03 §1](../specs/03-features-and-editor.md#1-bố-cục-giao-diện-chuẩn-3-pane-layout).

## Tiêu chí nghiệm thu

- [ ] Tạo bảng, thêm/xóa hàng-cột và định dạng ô bằng chuột và bàn phím hoạt động trên WinUI; nội dung bảng mở lại và đồng bộ sang thiết bị thứ hai đúng.
- [ ] Tag Browser lọc giao AND đúng khi chọn nhiều tag; danh sách tag tự cập nhật khi sửa ghi chú; không tag nào của ghi chú khóa xuất hiện.
- [ ] Smart Folder tạo từ form lọc đúng và cập nhật kết quả theo thời gian thực khi ghi chú thay đổi.
- [ ] Gallery View hiển thị thumbnail đúng, chấp nhận chuyển đổi qua lại mà không mất vị trí chọn; ghi chú khóa hiển thị theo chính sách.
- [ ] `Win+Alt+N` mở Quick Note từ ứng dụng bất kỳ; ghi chú nhanh lưu bền vững (kill tiến trình ngay sau khi báo lưu vẫn còn) và xuất hiện trong thư mục Quick Notes sau khi app focus lại.
- [ ] Filter chip kết hợp đúng với tìm kiếm và nhóm Pinned/thời gian; các thao tác mới không phá IME tiếng Việt và giữ vùng chọn khi chuyển ghi chú.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Bố cục chuẩn](../specs/03-features-and-editor.md#1-bố-cục-giao-diện-chuẩn-3-pane-layout) · [Client Windows](../specs/05-platform-clients-and-roadmap.md#2-windows-client-winui-3--windows-app-sdk)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành: toolbar bảng (tạo bảng 2×2, ±hàng/cột, di chuyển hàng/cột), Tag Browser sidebar lọc `#tag`, tạo Smart Folder qua form, Gallery View (thẻ tiêu đề/snippet), Quick Note Win+Alt+N (cửa sổ nổi, phiên riêng, lưu vào inbox), nút Xuất/Nhập.
- Kiểm chứng: build x64 xanh; nghiệm thu M1 6/6 PASS sau khi thêm UI.
- Còn lại: thumbnail Gallery (TD-018), tag pill render (TD-018), đếm ghi chú theo tag hiển thị trên sidebar.
