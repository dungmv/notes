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
    - `link`: `Option<Url | NoteLink(NoteId)>`
    - `tag`: `Option<String>` (`#tag`)
- **`tables`** (CRDT Map): Lưu cấu trúc hàng/cột và nội dung từng ô dưới dạng `CRDT Text`.

### Cơ chế Đồng bộ & Render
- Khi chỉnh sửa, UI gửi `EditorOp` (ví dụ: `InsertText`, `ToggleFormat`, `ToggleChecklist`, `MoveBlock`).
- `notes-crdt` chuyển `EditorOp` thành CRDT transaction, tạo ra `UpdateDelta (Vec<u8>)` nhỏ gọn để đẩy lên sync queue.
- UI nhận `RenderBlock[]` (hoặc delta patch của block thay đổi) để cập nhật màn hình.

---

## 3. Lưu trữ Cục bộ (Local Storage)

### 3.1. SQLite Schema (WAL Mode)

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
    entity_type TEXT NOT NULL, -- 'note' | 'folder' | 'attachment'
    entity_id TEXT NOT NULL,
    op_payload BLOB NOT NULL,
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
- **Lưu ý bảo mật**: Ghi chú đang khóa (`is_locked = 1`) **không** lưu `body_folded` vào `notes_fts` (chỉ có thể tìm theo tiêu đề nếu người dùng cho phép, hoặc ẩn hoàn toàn nội dung khỏi FTS index).

### 3.3. Content-Addressable Blob Store
- Thư mục dữ liệu trên từng OS:
  - **macOS**: `~/Library/Application Support/<AppBundleId>/`
  - **Windows**: `%LOCALAPPDATA%\<AppName>\`
  - **Linux**: `$XDG_DATA_HOME/<app-name>/`
- File đính kèm lưu tại `<data_dir>/blobs/<sha256[0..2]>/<sha256>`. Nếu ghi chú bị mã hóa, blob cũng được mã hóa bằng khóa của ghi chú đó trước khi lưu/sync.
