# 05. Đặc tả Client Đa nền tảng & Lộ trình Phát triển

## 1. macOS Client (SwiftUI + AppKit)

- **Phiên bản tối thiểu**: macOS 14.0+ (Sonoma) / macOS 15.0+ (Sequoia) để tận dụng tối đa `NavigationSplitView`, `@Observable`, và TextKit 2.
- **Tích hợp Native**:
  - **Layout**: `NavigationSplitView` 3 cột, hỗ trợ ẩn/hiện Sidebar, Toolbar chuẩn macOS.
  - **Editor**: `NSTextView` (TextKit 2) kết nối với `notes-core` qua `NotesCore.xcframework`.
  - **Phím tắt chuẩn Apple Notes**:
    - `Cmd+N`: Tạo ghi chú mới.
    - `Cmd+F` / `Cmd+Opt+F`: Tìm trong ghi chú / Tìm toàn bộ ghi chú.
    - `Cmd+Shift+L`: Tạo hoặc toggle Checklist.
    - `Cmd+Shift+T` / `H` / `B`: Title / Heading / Body.
    - `Cmd+K`: Chèn link hoặc `[[` liên kết ghi chú.
  - **Dịch vụ hệ thống**: Touch ID (`LocalAuthentication` + Keychain), Spotlight indexing (tùy chọn), Share Extension, Quick Note window.
  - **Phân phối**: DMG / Mac App Store (lưu ý App Sandbox entitlements cho thư mục dữ liệu và Network client).

---

## 2. Windows Client (WinUI 3 / Windows App SDK)

- **Phiên bản tối thiểu**: Windows 10 (1809+) / Windows 11 (khuyến nghị để có Mica/Fluent Design đầy đủ).
- **Công nghệ**: C# (.NET 8+), **WinUI 3** (`Microsoft.WindowsAppSDK`), giao tiếp với `notes_core.dll` qua UniFFI C# bindings (hoặc P/Invoke wrapper).
- **Thiết kế & UX (Fluent Design)**:
  - **Backdrop**: `MicaBackdrop` (chế độ tối/sáng tự động theo Windows Theme).
  - **Layout**: 3 cột sử dụng `NavigationView` (hoặc custom `Grid` + `SplitView` có thanh kéo chỉnh độ rộng cột).
  - **Phím tắt tương ứng**: `Ctrl+N`, `Ctrl+F`, `Ctrl+Shift+F`, `Ctrl+Shift+L`, `Ctrl+K`, v.v.
  - **Tích hợp hệ thống**:
    - **Windows Hello**: `UserConsentVerifier` + `PasswordVault` / DPAPI để mở khóa Locked Notes.
    - **System Tray / Quick Note**: Phím tắt toàn cục (ví dụ `Win+Alt+N`) mở cửa sổ Quick Note.
    - **Đóng gói**: MSIX package (hỗ trợ auto-update và Microsoft Store).

---

## 3. Kế hoạch Mở rộng Tương lai

### 3.1. Linux Client (Phase 3)
- **Đề xuất công nghệ**: **GTK4 + libadwaita** viết bằng **Rust (`gtk4-rs`)**.
  - **Lợi thế lớn**: Vì UI cũng viết bằng Rust, client Linux có thể **import trực tiếp crate `notes-app`**, không cần lớp FFI hay serialize dữ liệu trung gian.
  - Tích hợp `libsecret` (GNOME Keyring / KWallet) để lưu khóa bảo mật.
  - Đóng gói dưới dạng **Flatpak** (Flathub) và AppImage.

### 3.2. Mobile Clients: iOS & Android (Phase 4)
- **iOS / iPadOS (SwiftUI)**:
  - Tái sử dụng 100% Swift package `NotesCore` từ macOS.
  - Chuyển từ 3-pane sang `NavigationStack` (iPhone) và `NavigationSplitView` (iPad).
  - Tận dụng Face ID / Touch ID và Share Sheet.
- **Android (Jetpack Compose + Kotlin)**:
  - Dùng UniFFI Kotlin bindings + `libnotes_ffi.so` biên dịch qua `cargo-ndk` (cho `arm64-v8a`, `armeabi-v7a`, `x86_64`).
  - Bảo mật qua `BiometricPrompt` + Android Keystore.

---

## 4. Chiến lược Kiểm thử (Testing Strategy)

| Tầng | Loại Test | Mục tiêu |
| :--- | :--- | :--- |
| **Rust Core** | Unit & Property-based Tests (`proptest`) | Đảm bảo CRDT merge ngẫu nhiên luôn hội tụ (convergence), crypto encrypt/decrypt chuẩn xác, FTS5 tìm tiếng Việt đúng. |
| **Storage & Sync** | Integration Tests | Test SQLite migrations, rollback khi crash, sync 2 client giả lập (concurrent edits + offline conflicts). |
| **FFI Boundary** | Contract / Binding Tests | Chạy Swift test & C# xUnit test gọi trực tiếp vào `notes-ffi` để kiểm tra memory leak, UTF-16 offset, và async callbacks. |
| **UI Clients** | UI Automation Tests | XCUITest (macOS) và WinAppDriver / Appium (Windows) cho các luồng chính: tạo note, checklist, khóa note, tìm kiếm. |

---

## 5. Lộ trình Triển khai (Roadmap)

- **Phase 1 — Foundation & Core MVP**:
  - Khởi tạo Rust Workspace (`notes-domain`, `notes-crdt`, `notes-storage`, `notes-search`, `notes-app`, `notes-ffi`).
  - Hoàn thiện CRUD Folder, Note, Tag, FTS5 search, và UniFFI bindings cho Swift & C#.
- **Phase 2 — Desktop Clients (macOS & Windows v1.0)**:
  - Xây dựng giao diện 3 cột trên macOS (SwiftUI) và Windows (WinUI 3).
  - Hoàn thiện Rich Text Editor (styles, checklist, table, attachment), Locked Notes (Touch ID / Windows Hello), Export Markdown/PDF.
- **Phase 3 — Sync Engine & Linux**:
  - Triển khai Backend Service với **Java / Spring Boot + PostgreSQL + S3/MinIO** và adapter `notes-sync` (E2EE).
  - Ra mắt Linux client (GTK4 + libadwaita).
- **Phase 4 — Mobile Expansion**:
  - Phát triển iOS (SwiftUI) và Android (Jetpack Compose) dùng chung `notes-core` và hệ thống sync.
