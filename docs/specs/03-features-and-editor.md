# 03. Đặc tả Tính năng & Rich Text Editor

## 0. Hành vi tham chiếu và nghiệm thu

Apple Notes trên macOS là chuẩn hành vi. Ghi phiên bản hệ điều hành và loại tài khoản dùng khi đối chiếu vì hành vi có thể khác theo phiên bản và tài khoản. Hai client dùng chung nội dung tài liệu và kết quả thao tác. Cách trình bày, menu, phím tắt và điều hướng theo quy ước của từng hệ điều hành.

| Mốc | Luồng | Kết quả quan sát được |
| :--- | :--- | :--- |
| Phase 1–2 | Tạo, sửa và mở lại | Tạo note đưa focus vào editor. Gõ tiếng Việt bằng IME, sửa cạnh emoji, undo/redo và chuyển note giữ đúng vùng chọn và nội dung. Note đã báo lưu còn nguyên sau khi kill và mở lại app. |
| Phase 1–2 | Checklist | Return tạo mục tiếp theo, Shift+Return xuống dòng trong cùng mục, Tab/Shift+Tab đổi cấp khi ở trong danh sách. Khi bật tự sắp xếp, đánh dấu sẽ đưa mục xuống cuối và bỏ đánh dấu đưa mục về nhóm chưa hoàn tất. |
| Phase 2 | Đính kèm và tìm kiếm | Dán ảnh vào vị trí con trỏ, mở lại vẫn giữ ảnh đúng vị trí. Tìm không dấu trả về note tiếng Việt tương ứng. Phím tắt tìm trong note và tìm toàn bộ đưa focus vào đúng nơi. |
| Phase 2 | Liên kết ghi chú | Gõ `>>`, chọn note đích bằng bàn phím và mở được liên kết. Nhãn dùng tiêu đề tự cập nhật khi đổi tên note đích. Nhãn tùy chỉnh giữ nguyên. |
| Phase 1–2 | Canvas liền mạch | Một ghi chú hiển thị như một bề mặt văn bản liên tục: không có khung nhập rời cho từng đoạn; chọn, xóa và định dạng được xuyên nhiều đoạn; con trỏ đi qua bảng, ảnh và thẻ liên kết không dừng lại. Enter tạo đoạn mới trong cùng canvas, Backspace ở đầu đoạn gộp với đoạn trước. |
| Phase 3 | Cùng một tài liệu trên hai máy | Hai thiết bị sửa offline rồi kết nối lại. Cả hai nhận cùng nội dung, định dạng, trạng thái checklist và ảnh; chỉnh sửa độc lập ở các đoạn khác nhau đều được giữ. |
| Phase 4+ | Khóa ghi chú | Danh sách hiện tiêu đề và trạng thái khóa. Tìm theo nội dung, tag và bộ lọc attachment không tiết lộ dữ liệu đã khóa. Hủy xác thực hoặc khóa lại sẽ đưa editor về màn hình khóa và bỏ trạng thái phiên mở khóa. |

Nguồn đối chiếu [Checklist](https://support.apple.com/en-ca/guide/notes/apd93c815aa0/mac), [phím tắt](https://support.apple.com/guide/notes/keyboard-shortcuts-and-gestures-apd46c25187e/mac) và [liên kết ghi chú](https://support.apple.com/en-gb/guide/notes/apde615d29c2/mac).

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
- **Note Linking**: Gõ `>>` để mở danh sách gợi ý và chèn liên kết tới một ghi chú khác (`notes://note/<uuid>`). Hỗ trợ dùng tiêu đề hiện tại của ghi chú đích hoặc nhãn tùy chỉnh.
- **Inline Tags**: Gõ `#từ-khóa` tự động highlight thành tag pill và đồng bộ vào `note_tags` ngay khi kết thúc từ.

### 2.2. Đính kèm (Attachments)
- Kéo-thả hoặc dán (Paste) trực tiếp ảnh, PDF, audio, file bất kỳ vào ghi chú.
- Ảnh hỗ trợ xem cỡ lớn (Large) hoặc cỡ nhỏ (Small/Thumbnail) và mở Lightbox khi click đúp.
- PDF hỗ trợ xem inline trang hiện tại hoặc mở bằng trình xem mặc định.

### 2.3. Canvas soạn thảo liền mạch (Single-Canvas Editing)

Theo chuẩn Apple Notes, một ghi chú là **một bề mặt văn bản liên tục**, không phải tập hợp các ô nhập rời. Đây là yêu cầu trình bày bắt buộc cho mọi client:

- **Không viền input**: đoạn văn, checklist và tiêu đề nằm trong cùng canvas; không hiển thị khung hay đường viền cho từng block; đoạn trống chiếm đúng một dòng; khoảng cách giữa các đoạn chỉ là giãn dòng.
- **Tiêu đề**: dòng đầu áp kiểu Title trong cùng canvas (không phải ô tiêu đề riêng). Enter tại dòng tiêu đề tạo đoạn Body kế tiếp trong cùng canvas.
- **Con trỏ và vùng chọn tự do**: di chuyển, kéo chọn và định dạng xuyên nhiều đoạn trong cùng canvas; xóa vùng chọn nhiều đoạn gộp nội dung giữ ý định (undo trả về đúng trạng thái trước).
- **Checklist trong canvas**: mỗi mục checklist là một dòng trong canvas với checkbox bấm được; Enter tạo mục tiếp theo, Shift+Enter xuống dòng trong cùng mục, Tab/Shift+Tab đổi cấp.
- **Block nhúng trong dòng chảy**: bảng, ảnh và thẻ liên kết ghi chú là block object nằm trong luồng văn bản; con trỏ và vùng chọn đi qua chúng liền mạch, không cắt canvas thành các khung rời có viền riêng.
- **Cấu trúc dữ liệu không đổi**: canvas chỉ là lớp trình bày; mô hình block và hợp đồng thao tác CRDT (`replace`, split/merge, `paragraph_style`, `toggle`) giữ nguyên.

---

## 3. Chiến lược Kiến trúc Editor Đa nền tảng

Đây là phần kỹ thuật quan trọng nhất quyết định cảm giác "giống Apple Notes". Có 2 hướng tiếp cận chính:

### Phương án A: Native Text Engine per Platform (Khuyến nghị cho cảm giác Native tối đa)
- **macOS**: Dùng `NSTextView` (TextKit 2) bọc trong `NSViewRepresentable` cho SwiftUI. Tận dụng được spellcheck hệ thống, Writing Tools, dictation, dịch chuyển con trỏ mượt mà, VoiceOver và trackpad gestures chuẩn Apple.
- **Windows**: Dùng `RichEditBox` (WinUI 3) hoặc custom Win2D text surface. *Lưu ý*: `RichEditBox` của WinUI 3 có một số giới hạn với block phức tạp như Table lồng ghép hoặc inline Checklist pill tùy biến sâu.
- **Vai trò của Rust Core**: Rust giữ CRDT state và cung cấp **Incremental Diff / Patch API** để ánh xạ qua lại giữa vị trí ký tự (UTF-16 offset cho macOS/Windows ↔ UTF-8/CRDT offset trong Rust).

### Phương án B: Hybrid Shared Rich-Text Editor (WebView2 / WKWebView + ProseMirror/TipTap)
- Dùng một editor web nhúng (ProseMirror/Lexical) giao tiếp với `notes-core` qua bridge.
- **Ưu điểm**: Tái sử dụng cách render và xử lý thao tác bảng, checklist, highlight và kéo thả giữa các client. Mức tương đương vẫn cần được đo bằng các ca nghiệm thu.
- **Nhược điểm**: Tăng chi phí bộ nhớ và có thể ảnh hưởng dịch vụ văn bản native. So sánh bằng prototype trên cùng tài liệu và máy kiểm thử.

### Quyết định Kiến trúc Đề xuất
- **Phase 1**. Thử `NSTextView` với TextKit 2 và WinUI 3 `RichEditBox` trên cùng một tài liệu thử, cùng hợp đồng `EditorOp`/patch qua Rust. Kiểm tra IME, emoji, selection, undo/redo, checklist và một bảng cơ bản. Xác minh TextKit 2 vẫn hoạt động trong các luồng thử.
- **Quyết định Windows**. Ưu tiên `RichEditBox` nếu đạt ca thử. Nếu có ca bắt buộc chưa đạt, ghi lại hành vi cụ thể, thử WebView2 cục bộ cho editor và đo lại các ca đó trước khi chốt adapter. Kết quả thử xác định hợp đồng dữ liệu và FFI trước khi hoàn thiện cả hai client.
- **Chuẩn hóa Offset**: Mọi API giao tiếp giữa Editor UI và `notes-crdt` phải quy định rõ hệ quy chiếu offset (UTF-16 code units cho Swift `NSString` và C# `string`; UTF-8 byte offset cho Rust/GTK4). `notes-ffi` cung cấp helper chuyển đổi chính xác để tránh lệch con trỏ khi gõ tiếng Việt có dấu hoặc Emoji.

### Kết quả chốt Windows: canvas phân khúc (M4-W.9)

Editor Windows chốt theo phương án A, nhưng chuyển lớp projection từ "một control cho từng block" sang **canvas phân khúc (segmented canvas)** để đạt hành vi [§2.3](#23-canvas-soạn-thảo-liền-mạch-single-canvas-editing):

- Chuỗi paragraph/checklist **liên tiếp** trong tài liệu được chiếu vào **một** `RichEditBox` duy nhất: TOM paragraph ↔ block, ánh xạ offset UTF-16 (tiếng Việt, emoji) giữa TOM và hợp đồng block qua helper `notes-ffi`. Bảng, ảnh và thẻ liên kết vẫn là control nhúng và tách run — canvas "gãy" tại block phi văn bản, con trỏ và vùng chọn phải được chuyển vùng liền mạch qua ranh giới này.
- Style đoạn (Title, Heading, Subheading, Body, Quote, Monospaced) áp bằng `CharacterFormat`/`ParagraphFormat` trong TOM; checkbox checklist là glyph cục bộ đầu dòng, không thuộc nội dung CRDT, bấm được qua hit-test.
- Lý do không giữ control rời cho từng block: trải nghiệm "một ghi chú nhiều ô nhập" (TD-020), mất chọn/xóa xuyên đoạn, IME chịu ranh giới control. Lý do chưa chuyển WebView2: hợp đồng thao tác và projection native đã qua các ca M0–M4; WebView2 chỉ mở lại nếu canvas phân khúc không đạt một ca bắt buộc nào đó — thử theo đúng quy trình mục trên và đo lại trước khi chốt.

---

## 4. Ghi chú Khóa (Locked Notes) & Thao tác Nhanh

- **Locked Notes**:
  - Khóa từng ghi chú bằng mật khẩu khóa, có tùy chọn mở nhanh qua **Touch ID** (macOS) hoặc **Windows Hello** (Windows). Chính sách tiêu đề, tìm kiếm và đính kèm theo [spec lưu trữ](./02-data-model-and-storage.md#34-chính-sách-lưu-trữ-locked-notes).
  - Tự động khóa lại (auto-lock) khi: bấm nút Lock Now, khóa màn hình máy tính, hoặc sau N phút không hoạt động.
- **Quick Note**:
  - Phím tắt toàn cục (`Global Hotkey`) mở cửa sổ ghi chú nổi (Floating Window) từ bất kỳ ứng dụng nào để ghi nhanh.
- **Import / Export**:
  - Export ghi chú sang **Markdown (`.md` + folder ảnh)**, **HTML**, và **PDF**.
  - Import từ Markdown folder và Apple Notes (qua HTML/RTFD export hoặc parser).
