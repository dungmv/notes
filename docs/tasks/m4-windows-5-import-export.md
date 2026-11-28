# M4-W.5 · Import/Export và ma trận giữ trường

**Phụ thuộc** — [M4-W.3](./m4-windows-3-tables-tags-smart-folders-core.md) (nội dung bảng, tag và thư mục con nằm trong phạm vi export/import).

## Phạm vi

- Export từng ghi chú và cả thư mục: **Markdown** (`.md` + thư mục ảnh kèm theo), **HTML**, **PDF**; với mỗi định dạng quy định rõ phạm vi giữ trường (nội dung, định dạng inline, checklist, bảng, ảnh, liên kết ghi chú, tag) và phần nằm ngoài phạm vi theo [03 §4](../specs/03-features-and-editor.md#4-ghi-chú-khóa-locked-notes--thao-tác-nhanh).
- Import: **Markdown folder** (cây thư mục → cây folder, ảnh kèm theo → blob store, frontmatter nếu có) và **Apple Notes** qua bản export HTML/RTFD chạy được trên Windows; mapping thư mục, tag, ngày tạo/sửa và ảnh; ghi chú khóa không nằm trong export/import.
- Bộ fixture dùng chung `fixtures/m4`: tài liệu chứa đủ loại block (title, heading, list, checklist, bảng, ảnh, liên kết ghi chú, tag, tiếng Việt có dấu, emoji) làm dữ liệu đối chiếu cho cả hai chiều import→export và export→import.
- Ma trận giữ trường: bảng *trường × định dạng* với kết quả giữ/mất/giảm chất lượng và lý do cho từng ô, dùng bộ fixture để chứng minh theo yêu cầu roadmap M4; tài liệu hóa các quyết định phạm vi.
- PDF: xuất qua đường in hệ thống hoặc thư viện; kiểm tra font tiếng Việt, ngắt trang và ảnh trong bản PDF kết quả.
- Export/import chạy qua Rust core để hợp đồng giữ trường dùng chung cho các client sau này (macOS, M5).

## Tiêu chí nghiệm thu

- [ ] Export Markdown/HTML/PDF từ bộ fixture tạo được bản ghi mở lại bằng công cụ ngoài (editor text/trình duyệt/PDF reader) với đúng nội dung và định dạng trong phạm vi đã công bố.
- [ ] Import Markdown folder và Apple Notes HTML tái tạo đúng cây thư mục, ghi chú, checklist, bảng, ảnh và tag; import hai lần không nhân bản ghi chú.
- [ ] Ma trận giữ trường hoàn chỉnh cho ba định dạng, mỗi ô có bằng chứng từ fixture; các trường mất/giảm có ghi rõ lý do và trạng thái (đã ghi nợ nếu cần xử lý sau).
- [ ] Export/import không phá vỡ dữ liệu hiện có: import vào thư viện đang sync đưa ghi chú mới vào outbox đúng cơ chế M3; ghi chú khóa bị loại khỏi cả export và import.
- [ ] PDF xuất tiếng Việt đúng font, không lỗi ký tự; ảnh trong export giữ độ phân giải chấp nhận được và được ghi đúng vào thư mục/blob kèm theo.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Editor spec](../specs/03-features-and-editor.md#4-ghi-chú-khóa-locked-notes--thao-tác-nhanh) · [Lưu trữ](../specs/02-data-model-and-storage.md#3-lưu-trữ-cục-bộ-local-storage)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành: core `transfer.rs` — export Markdown (.md + attachments/), HTML, PDF (font Arial nhúng, hỗ trợ tiếng Việt); import Markdown folder (thư mục con, ảnh → blob store, checklist/heading/bảng pipe/inline marks/note link) và Apple Notes HTML; FFI `export_note`/`import_folder`; UI Xuất/Nhập với FolderPicker.
- Ma trận giữ trường: test round-trip xác nhận giữ nội dung, bold, checklist, bảng, ảnh, tag; mất/giảm: underline (md), highlight (md/html), ảnh trong PDF, note-link import giữ dạng nhãn tùy chỉnh — ghi nhận TD-018.
- Còn lại: fixture `fixtures/m4` dùng chung hai chiều cho hai client khi macOS đạt nền M1–M3.
