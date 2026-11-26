# 03. Đặc tả Tính năng & Rich Text Editor

## 1. Bố cục Giao diện Chuẩn (3-Pane Layout)

Tương tự Apple Notes, ứng dụng sử dụng bố cục 3 cột có thể thu gọn linh hoạt:

1. **Cột 1 — Sidebar (Folders & Tags)**:
   - **System Folders**: All Notes, Quick Notes (tùy chọn), Shared (tương lai), Recently Deleted (30 ngày).
   - **User Folders**: Hỗ trợ kéo-thả sắp xếp, lồng thư mục con, đổi tên, gán icon.
   - **Smart Folders**: Tự động nhóm ghi chú theo điều kiện (ví dụ: `#work AND has:checklist`).
   - **Tag Browser**: Danh sách các `#tag` đang có, hỗ trợ chọn nhiều tag để lọc giao (AND).
2. **Cột 2 — Note List**:
   - Thanh tìm kiếm nhanh (Search bar) hỗ trợ filter chip (`Locked`, `Shared`, `Checklists`, `Attachments`, `#tag`).
   - Nhóm **Pinned** luôn nằm trên cùng, phía dưới phân nhóm theo thời gian (`Today`, `Previous 7 Days`, `Previous 30 Days`, theo tháng/năm).
   - Hiển thị: Tiêu đề (1 dòng), Ngày cập nhật + Snippet (1–2 dòng), Thumbnail nhỏ nếu có ảnh, icon khóa nếu là Locked Note.
   - Hỗ trợ chuyển đổi chế độ xem **List View** và **Gallery View** (dạng thẻ giống Apple Notes).
3. **Cột 3 — Editor**:
   - Toolbar định dạng gọn gàng: Checklist, Format (`Aa`), Table, Attachment, Link Note, Lock, Share/Export, Pin, Info (số từ, ký tự, ngày tạo/sửa).

---

## 2. Tính năng Soạn thảo (Editor Capabilities)

### 2.1. Kiểu chữ & Block Styles
- **Paragraph Styles**: Title, Heading, Subheading, Body, Monospaced, Block Quote.
- **Inline Formatting**: Bold (`Cmd/Ctrl+B`), Italic (`Cmd/Ctrl+I`), Underline (`Cmd/Ctrl+U`), Strikethrough, Highlight (nhiều màu), Inline Code, Hyperlink.
- **Lists**:
  - Bulleted List (`- ` hoặc `* ` tự động chuyển đổi khi gõ).
  - Dashed List.
  - Numbered List (`1. `).
  - **Interactive Checklists**: Gõ `[] ` để tạo nhanh; bấm `Enter` để tạo dòng checklist mới; hỗ trợ cài đặt **"Automatically sort checked items"** (tự động đẩy mục đã tick xuống cuối danh sách).
- **Tables**: Tạo bảng, thêm/xóa/kéo thả hàng và cột, định dạng text bên trong ô.
- **Note Linking**: Gõ `[[` hoặc `>>` để mở popup tìm kiếm và chèn liên kết tới một ghi chú khác (`notes://note/<uuid>`).
- **Inline Tags**: Gõ `#từ-khóa` tự động highlight thành tag pill và đồng bộ vào `note_tags` ngay khi kết thúc từ.

### 2.2. Đính kèm (Attachments)
- Kéo-thả hoặc dán (Paste) trực tiếp ảnh, PDF, audio, file bất kỳ vào ghi chú.
- Ảnh hỗ trợ xem cỡ lớn (Large) hoặc cỡ nhỏ (Small/Thumbnail) và mở Lightbox khi click đúp.
- PDF hỗ trợ xem inline trang hiện tại hoặc mở bằng trình xem mặc định.

---

## 3. Chiến lược Kiến trúc Editor Đa nền tảng

Đây là phần kỹ thuật quan trọng nhất quyết định cảm giác "giống Apple Notes". Có 2 hướng tiếp cận chính:

### Phương án A: Native Text Engine per Platform (Khuyến nghị cho cảm giác Native tối đa)
- **macOS**: Dùng `NSTextView` (TextKit 2) bọc trong `NSViewRepresentable` cho SwiftUI. Tận dụng được spellcheck hệ thống, Writing Tools, dictation, dịch chuyển con trỏ mượt mà, VoiceOver và trackpad gestures chuẩn Apple.
- **Windows**: Dùng `RichEditBox` (WinUI 3) hoặc custom Win2D text surface. *Lưu ý*: `RichEditBox` của WinUI 3 có một số giới hạn với block phức tạp như Table lồng ghép hoặc inline Checklist pill tùy biến sâu.
- **Vai trò của Rust Core**: Rust giữ CRDT state và cung cấp **Incremental Diff / Patch API** để ánh xạ qua lại giữa vị trí ký tự (UTF-16 offset cho macOS/Windows ↔ UTF-8/CRDT offset trong Rust).

### Phương án B: Hybrid Shared Rich-Text Editor (WebView2 / WKWebView + ProseMirror/TipTap)
- Dùng một editor web nhúng (ProseMirror/Lexical) giao tiếp với `notes-core` qua bridge.
- **Ưu điểm**: Đồng nhất 100% hành vi Table, Checklist, Highlight, Drag-drop block trên cả macOS, Windows, Linux và Mobile; tốc độ phát triển tính năng mới rất nhanh.
- **Nhược điểm**: Tốn thêm RAM (~40–80MB cho mỗi WebView process), mất một phần tích hợp native text service nếu không tinh chỉnh kỹ.

### Quyết định Kiến trúc Đề xuất
- **Giai đoạn đầu (MVP → v1.0)**: Dùng **Native Editor trên macOS (`NSTextView` / TextKit 2)** để đạt chuẩn Apple Notes, và **WinUI 3 `RichEditBox` (hoặc WebView2 cục bộ cho Editor pane trên Windows nếu `RichEditBox` không đáp ứng đủ Table/Checklist UX)** — tách `EditorView` thành một interface độc lập trong client để có thể nâng cấp từng nền tảng mà không ảnh hưởng `notes-core`.
- **Chuẩn hóa Offset**: Mọi API giao tiếp giữa Editor UI và `notes-crdt` phải quy định rõ hệ quy chiếu offset (UTF-16 code units cho Swift `NSString` và C# `string`; UTF-8 byte offset cho Rust/GTK4). `notes-ffi` cung cấp helper chuyển đổi chính xác để tránh lệch con trỏ khi gõ tiếng Việt có dấu hoặc Emoji.

---

## 4. Ghi chú Khóa (Locked Notes) & Thao tác Nhanh

- **Locked Notes**:
  - Hỗ trợ khóa từng ghi chú bằng mật khẩu riêng (hoặc mật khẩu tài khoản) kết hợp **Touch ID** (macOS) / **Windows Hello** (Windows).
  - Tự động khóa lại (auto-lock) khi: bấm nút Lock Now, khóa màn hình máy tính, hoặc sau N phút không hoạt động.
- **Quick Note**:
  - Phím tắt toàn cục (`Global Hotkey`) mở cửa sổ ghi chú nổi (Floating Window) từ bất kỳ ứng dụng nào để ghi nhanh.
- **Import / Export**:
  - Export ghi chú sang **Markdown (`.md` + folder ảnh)**, **HTML**, và **PDF**.
  - Import từ Markdown folder và Apple Notes (qua HTML/RTFD export hoặc parser).
