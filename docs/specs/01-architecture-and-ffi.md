# 01. Kiến trúc Hệ thống & FFI Bridge

## 1. Tổng quan Kiến trúc

Hệ thống tuân theo mô hình **Shared Core + Native Shells**:

```text
+-----------------------+   +-----------------------+   +-----------------------+
|   macOS (SwiftUI)     |   |  Windows (WinUI 3)    |   |  Linux / Mobile UI    |
|  AppKit + TextKit 2   |   |  C# + Windows App SDK |   | GTK4 / Compose / iOS  |
+-----------+-----------+   +-----------+-----------+   +-----------+-----------+
            |                           |                           |
            | UniFFI (Swift)            | UniFFI C# / csbindgen     | Direct / UniFFI
            v                           v                           v
+-------------------------------------------------------------------------------+
|                         notes-ffi (FFI Boundary Layer)                        |
|        Commands (Sync/Async)  |  DTOs  |  Event Stream / Observer Callback    |
+-------------------------------------------------------------------------------+
|                              notes-core (Rust)                                |
|  +--------------+  +--------------+  +--------------+  +-------------------+  |
|  | notes-domain |  |  notes-crdt  |  |notes-storage |  |    notes-sync     |  |
|  +--------------+  +--------------+  +--------------+  +-------------------+  |
|  | notes-search |  | notes-crypto |  | notes-export |  |  notes-telemetry  |  |
|  +--------------+  +--------------+  +--------------+  +-------------------+  |
+-------------------------------------------------------------------------------+
```

---

## 2. Cấu trúc Rust Workspace

Tổ chức repo theo dạng monorepo:

```text
notes/
├── Cargo.toml                  # Rust workspace root
├── core/
│   ├── notes-domain/           # Entities, Value Objects, Errors, Domain Rules
│   ├── notes-crdt/             # Rich-text CRDT engine (yrs / Loro wrapper), AST & Operations
│   ├── notes-storage/          # SQLite (rusqlite/sqlx), migrations, Content-Addressable Blob Store
│   ├── notes-search/           # FTS5 query builder, Vietnamese diacritic folding, Smart Folder eval
│   ├── notes-crypto/           # Argon2id, AES-256-GCM / XChaCha20-Poly1305, Key wrapping
│   ├── notes-sync/             # Sync state machine, Delta push/pull, Blob chunking, Conflict resolution
│   ├── notes-export/           # Markdown, HTML, PDF/Print AST exporters & importers
│   ├── notes-app/              # Application Service / Facade (AppCore), Tokio runtime management
│   └── notes-ffi/              # UniFFI & C-ABI bindings generation
├── clients/
│   ├── macos/                  # Xcode project (SwiftUI + AppKit)
│   ├── windows/                # Visual Studio solution (WinUI 3 / C#)
│   ├── linux/                  # (Future) GTK4 / libadwaita
│   └── mobile/                 # (Future) iOS (SwiftUI) & Android (Kotlin)
├── docs/
│   └── specs/
└── scripts/                    # Build scripts tạo XCFramework, DLL, bindings
```

### Trách nhiệm từng Crate

- **`notes-domain`**: Không phụ thuộc I/O. Chứa `NoteId`, `FolderId`, `Tag`, `SmartFolderQuery`, `NoteMetadata`, `RichTextOp`, và các domain error.
- **`notes-crdt`**: Đóng gói thư viện CRDT (khuyến nghị **`yrs`** hoặc **`loro`**). Quản lý cấu trúc cây block/text của ghi chú, chuyển đổi giữa CRDT state ↔ `DocumentSnapshot` (JSON/Binary) ↔ `RenderBlock[]` để UI vẽ.
- **`notes-storage`**: Quản lý SQLite (WAL mode, connection pool), schema migrations, và lưu file đính kèm theo SHA-256 hash (`blobs/<hash[0..2]>/<hash>`).
- **`notes-search`**: Xây dựng chỉ mục FTS5, chuẩn hóa chuỗi tiếng Việt (tìm `"ghi chu"` khớp cả `"ghi chú"`), lọc theo `#tag`, checklist, attachment.
- **`notes-crypto`**: Xử lý mã hóa Locked Notes và E2EE Sync. Không bao giờ lưu plaintext key trên disk.
- **`notes-sync`**: Giao tiếp qua trait `SyncTransport` (HTTP/REST + Centrifugo real-time client), quản lý sync cursor, retry backoff, và upload/download blob.
- **`notes-app`**: Điểm vào duy nhất cho mọi client (`NotesEngine`). Sở hữu một `tokio::runtime::Runtime` nội bộ để chạy I/O bất đồng bộ mà không bắt UI phải tự quản lý async runtime của Rust.

---

## 3. Chiến lược FFI (Foreign Function Interface)

### 3.1. Công cụ Binding

- **macOS & iOS (Swift)**: Dùng **Mozilla UniFFI** (`uniffi-bindgen generate --language swift`). Tạo ra `NotesCore.xcframework` (Universal binary `arm64` + `x86_64` cho macOS, và `arm64` cho iOS).
- **Android (Kotlin)**: Dùng **UniFFI** (`--language kotlin`).
- **Windows (C# / WinUI 3)**:
  - **Lựa chọn ưu tiên**: `uniffi-bindgen-cs` (cộng đồng UniFFI C# generator) để dùng chung một file định nghĩa UDL / proc-macro `#[uniffi::export]` với Swift và Kotlin.
  - **Phương án dự phòng**: Nếu cần tối ưu zero-copy cho buffer lớn hoặc kiểm soát P/Invoke chặt chẽ trên Windows Native AOT, dùng `csbindgen` xuất C-ABI trực tiếp từ `notes-ffi` thành `notes_core.dll`.
- **Linux**: Nếu viết UI bằng `gtk4-rs` (Rust), gọi trực tiếp `notes-app` không qua FFI. Nếu dùng ngôn ngữ khác, dùng C-header từ `cbindgen`.

### 3.2. Thiết kế API qua FFI

Tránh gọi FFI quá dày đặc cho từng ký tự gõ phím nếu có chi phí serialize, hoặc giữ payload nhỏ gọn (protobuf/bincode hoặc flat UniFFI records):

1. **Command API (UI → Core)**:
   - `initialize(config: EngineConfig) -> Result<NotesEngine>`
   - `list_folders() -> Vec<FolderDto>`
   - `list_notes(filter: NoteFilter, sort: SortOrder) -> Vec<NoteSummaryDto>`
   - `open_note(note_id: String) -> Result<NoteDetailDto>`
   - `apply_editor_ops(note_id: String, ops: Vec<EditorOp>) -> Result<EditorUpdateDto>`
   - `unlock_note(note_id: String, secret: UnlockSecret) -> Result<NoteDetailDto>`
   - `trigger_sync() -> Result<()>`
2. **Observer / Event Callback (Core → UI)**:
   - UI đăng ký một `CoreEventListener` (UniFFI callback interface).
   - Khi sync nền hoàn tất, hoặc một ghi chú thay đổi từ thiết bị khác, hoặc tiến trình index tìm kiếm cập nhật, Rust Core phát sự kiện:
     - `FoldersChanged`
     - `NoteListInvalidated { folder_id }`
     - `NoteContentUpdated { note_id, delta_blocks }`
     - `SyncStateChanged { state: SyncStatus }`
     - `ErrorOccurred { code, message }`

---

## 4. Threading & Async Model

- **UI Thread không bao giờ bị block**: Mọi thao tác đọc/ghi DB nặng, giải mã, OCR/thumbnail, hoặc network sync đều chạy trên Tokio worker threads bên trong `notes-app`.
- **Dispatch về Main Thread**: Callback từ Rust sang Swift/C#/Kotlin luôn được gọi trên background thread của Rust. Lớp wrapper mỏng ở phía client (Swift `Actor`/`@MainActor`, C# `DispatcherQueue.TryEnqueue`) có trách nhiệm chuyển sự kiện về UI thread trước khi cập nhật Observable state.
- **Debouncing & Batching**:
  - Khi người dùng gõ phím liên tục, UI cập nhật tức thì trên buffer cục bộ (hoặc optimistic state) và gửi `EditorOp` sang Rust Core.
  - Rust Core áp dụng vào CRDT in-memory ngay lập tức, nhưng **debounce việc flush snapshot xuống SQLite** (ví dụ: mỗi 300ms–500ms hoặc khi blur/chuyển note) để tránh I/O bottleneck.
