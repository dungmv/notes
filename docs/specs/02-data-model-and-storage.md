# 02. Mô hình Dữ liệu & Lưu trữ

## 1. Thực thể Cốt lõi (Domain Entities)

Thiết kế mô hình dữ liệu tương thích với các khái niệm của Apple Notes:

### 1.1. `Folder`
- `id`: `UUIDv7` (hỗ trợ sắp xếp theo thời gian tạo tự nhiên).
- `parent_id`: `Option<UUIDv7>` (hỗ trợ thư mục lồng nhau nhiều cấp).
- `name`: `String`
- `icon`: `Option<String>` (mapping sang SF Symbols trên macOS / Segoe Fluent Icons trên Windows).
- `kind`: `Normal | SmartFolder | SystemAllNotes | SystemTrash`
- `smart_query`: `Option<SmartFolderQuery>` (JSON DSL lọc theo tag, ngày sửa, có attachment, có checklist, trạng thái khóa).
- `sort_order`: `i64`
- `created_at`, `updated_at`, `deleted_at`: `i64` (UTC milliseconds).

### 1.2. `Note`
- `id`: `UUIDv7`
- `folder_id`: `UUIDv7`
- `title`: `String` (tự động trích xuất từ dòng đầu tiên / heading đầu tiên của nội dung, giống Apple Notes).
- `snippet`: `String` (đoạn text tóm tắt 1–2 dòng hiển thị ở danh sách ghi chú).
- `is_pinned`: `bool`
- `is_locked`: `bool`
- `lock_metadata`: `Option<NoteLockMetadata>` (salt, KDF params, wrapped note key, biometric hint).
- `crdt_state`: `Vec<u8>` (trạng thái nhị phân của CRDT document — được mã hóa nếu `is_locked = true`).
- `plain_text_cache`: `String` (bản text thuần để đánh chỉ mục FTS5; rỗng nếu `is_locked = true` để bảo mật).
- `has_checklist`, `has_attachments`: `bool`
- `created_at`, `updated_at`, `deleted_at`: `i64` (nếu nằm trong Recently Deleted, tự động xóa vĩnh viễn sau 30 ngày).

### 1.3. `Tag`
- Trích xuất tự động từ nội dung ghi chú theo cú pháp `#tag` (hỗ trợ Unicode/tiếng Việt, ví dụ `#cong-viec`, `#hoc_tap`).
- Bảng `note_tags(note_id, tag_name)` được cập nhật mỗi khi CRDT document thay đổi.

### 1.4. `Attachment`
- `id`: `UUIDv7`
- `note_id`: `UUIDv7`
- `kind`: `Image | Pdf | Audio | File | Sketch`
- `file_name`: `String`
- `mime_type`: `String`
- `byte_size`: `i64`
- `content_hash`: `String` (SHA-256 dùng để deduplicate trong Blob Store).
- `thumbnail_hash`: `Option<String>` (ảnh thu nhỏ tạo sẵn cho ảnh/PDF).
- `ocr_text`: `Option<String>` (tùy chọn mở rộng cho tìm kiếm văn bản trong ảnh/PDF).

---

## 2. Rich Text & CRDT Document Model

Để hỗ trợ chỉnh sửa mượt mà và đồng bộ nhiều thiết bị không xung đột (ví dụ: sửa cùng một ghi chú trên macOS khi offline và trên Windows khi online), nội dung ghi chú được biểu diễn bằng **CRDT** (khuyến nghị **`yrs`** — bản Rust của Yjs, hoặc **`loro`**).

### Cấu trúc Document trong CRDT
Mỗi `Note` chứa một CRDT Document gồm:
- **`blocks`** (CRDT List / Array of Maps): Danh sách các block theo thứ tự:
  - `id`: `BlockId`
  - `type`: `Title | Heading | Subheading | Paragraph | Monospaced | BlockQuote | BulletList | DashedList | NumberedList | Checklist | Table | AttachmentRef | Divider`
  - `indent_level`: `u8` (0..6)
  - `checked`: `Option<bool>` (dành cho `Checklist`)
  - `attachment_id`: `Option<UUIDv7>` (dành cho `AttachmentRef`)
  - `text`: `CRDT XmlText / RichText` hỗ trợ inline attributes:
    - `bold`, `italic`, `underline`, `strikethrough`, `code`, `highlight_color`
    - `link`: `Option<Url | NoteLink(NoteId, LabelMode)>`. Khi `LabelMode` dùng tiêu đề, client hiển thị tiêu đề hiện tại của đích theo ID. Nhãn tùy chỉnh là một phần nội dung tài liệu.
    - `tag`: `Option<String>` (`#tag`)
- **`tables`** (CRDT Map): Lưu cấu trúc hàng/cột và nội dung từng ô dưới dạng `CRDT Text`.

### Cơ chế Đồng bộ & Render
- Khi chỉnh sửa, UI gửi `EditorOp` (ví dụ: `InsertText`, `ToggleFormat`, `ToggleChecklist`, `MoveBlock`).
- `notes-crdt` chuyển `EditorOp` thành CRDT transaction, tạo ra `UpdateDelta (Vec<u8>)` nhỏ gọn để đẩy lên sync queue.
- UI nhận `RenderBlock[]` (hoặc delta patch của block thay đổi) để cập nhật màn hình.

---

## 3. Lưu trữ Cục bộ (Local Storage)

### 3.1. Lưu bền vững và khôi phục

UI hiển thị thay đổi ngay và chỉ báo đã lưu sau khi nhận xác nhận từ core. SQLite dùng WAL và `synchronous=FULL` trên kết nối ghi. Core duy trì thứ tự thao tác và xác nhận một batch sau khi transaction cập nhật trạng thái tài liệu, các trường dẫn xuất và outbox (khi bật sync) đã commit. Mỗi mục outbox giữ `client_op_id` ổn định. Khi mã hóa lần đầu, lưu envelope và phiên bản khóa vào outbox trước khi gửi để retry dùng đúng cùng dữ liệu. Lần đầu bật sync, core đưa trạng thái hiện có vào outbox để ghi chú tạo trước đó cũng được đồng bộ, theo điều kiện áp dụng Locked Notes trong spec E2EE.

Khi ghi thất bại, transaction rollback và core giữ trạng thái đã commit làm cơ sở thử lại. Thao tác chưa được xác nhận có thể mất nếu tiến trình bị dừng đột ngột. Ca nghiệm thu kill tiến trình sau xác nhận, mở lại và kiểm tra nội dung, ảnh và outbox vẫn nhất quán. Với attachment, core hoàn tất ghi bền vững file rồi mới commit tham chiếu. File mồ côi do gián đoạn có thể được dọn lại khi khởi động.

SQLite schema cho các bảng cốt lõi:

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE folders (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES folders(id),
    name TEXT NOT NULL,
    icon TEXT,
    kind TEXT NOT NULL,
    smart_query_json TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    folder_id TEXT NOT NULL REFERENCES folders(id),
    title TEXT NOT NULL DEFAULT '',
    snippet TEXT NOT NULL DEFAULT '',
    is_pinned INTEGER NOT NULL DEFAULT 0,
    is_locked INTEGER NOT NULL DEFAULT 0,
    lock_metadata_blob BLOB,
    crdt_state BLOB NOT NULL,
    plain_text_cache TEXT NOT NULL DEFAULT '',
    has_checklist INTEGER NOT NULL DEFAULT 0,
    has_attachments INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX idx_notes_folder_updated ON notes(folder_id, is_pinned DESC, updated_at DESC) WHERE deleted_at IS NULL;
CREATE INDEX idx_notes_deleted ON notes(deleted_at) WHERE deleted_at IS NOT NULL;

CREATE TABLE note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag TEXT NOT NULL,
    PRIMARY KEY (note_id, tag)
);
CREATE INDEX idx_note_tags_tag ON note_tags(tag);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    thumbnail_hash TEXT,
    created_at INTEGER NOT NULL
);

CREATE TABLE sync_oplog (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    client_op_id TEXT NOT NULL UNIQUE,
    entity_type TEXT NOT NULL, -- 'note' | 'folder' | 'attachment_meta'
    entity_id TEXT NOT NULL,
    op_payload BLOB NOT NULL,
    encrypted_envelope BLOB, -- Envelope đã chuẩn bị, dùng lại qua retry
    created_at INTEGER NOT NULL,
    synced_at INTEGER
);
```

### 3.2. Full-Text Search (FTS5)

```sql
CREATE VIRTUAL TABLE notes_fts USING fts5(
    note_id UNINDEXED,
    title,
    body_folded, -- Text đã chuẩn hóa bỏ dấu tiếng Việt (ví dụ: "ghi chú" -> "ghi chu")
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);
```
- FTS5 chỉ chứa ghi chú thường. Tìm theo tiêu đề ghi chú khóa dùng cột `notes.title` qua truy vấn riêng.

### 3.3. Content-Addressable Blob Store
- Thư mục dữ liệu trên từng OS:
  - **macOS**: `~/Library/Application Support/<AppBundleId>/`
  - **Windows**: `%LOCALAPPDATA%\<AppName>\`
  - **Linux**: `$XDG_DATA_HOME/<app-name>/`
- Blob của ghi chú thường dùng đường dẫn cục bộ `<data_dir>/blobs/<sha256[0..2]>/<sha256>`. Định danh blob sync theo [spec E2EE](./07-e2ee-cryptography.md#52-deduplication-an-toàn-per-user-keyed-convergent-encryption). Blob khóa dùng ID và khóa mã hóa ngẫu nhiên.

### 3.4. Chính sách lưu trữ Locked Notes

- Bản chiếu khi khóa được phép hiện tiêu đề, vị trí thư mục, trạng thái ghim/khóa và mốc thời gian. Tiêu đề có thể dùng trong kết quả tìm theo tiêu đề. Danh sách để trống snippet và thumbnail. Bộ lọc nội dung, tag, checklist và attachment bỏ qua ghi chú khóa.
- Khi chuyển ghi chú thường sang ghi chú khóa, core ghi phiên bản đã mã hóa của nội dung và manifest attachment. Transaction xóa bản ghi FTS, tag, metadata attachment dạng rõ, snippet, cache văn bản và các cờ suy ra từ nội dung. Trước khi commit, blob khóa phải được ghi bền vững; sau commit, dọn bản rõ cục bộ khi không còn ghi chú thường nào tham chiếu. Cache UI, preview và khóa phiên mở được giải phóng khi khóa lại.
- Phiên mở khóa chỉ dựng nội dung, attachment và dữ liệu dẫn xuất trong bộ nhớ; các bản chiếu trên đĩa vẫn theo chính sách trên. Chỉ thao tác gỡ bảo vệ mới khôi phục các bản ghi tìm kiếm và metadata.
- Phạm vi bảo vệ áp dụng cho bản lưu của ghi chú khóa. Bản sao độc lập đã xuất hoặc đã được tạo trước khi khóa nằm ngoài phạm vi. Xóa blob cục bộ là xóa ở tầng ứng dụng; thu hồi các bản sao lịch sử và xóa vật lý khỏi thiết bị lưu trữ cần cơ chế riêng. SQLCipher bảo vệ file SQLite và WAL ở tầng đĩa, với giới hạn riêng cho blob store. Yêu cầu nghiệm thu và điều kiện bật tính năng nằm trong [spec E2EE](./07-e2ee-cryptography.md#71-mã-hóa-cơ-sở-dữ-liệu-cục-bộ).
