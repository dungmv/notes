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
    - `Cmd+K`: Chèn liên kết. Gõ `>>` trong editor để gợi ý ghi chú đích.
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

### 3.1. Linux Client (Phase 4+)
- **Đề xuất công nghệ**: **GTK4 + libadwaita** viết bằng **Rust (`gtk4-rs`)**.
  - **Lợi thế lớn**: Vì UI cũng viết bằng Rust, client Linux có thể **import trực tiếp crate `notes-app`**, không cần lớp FFI hay serialize dữ liệu trung gian.
  - Tích hợp `libsecret` (GNOME Keyring / KWallet) để lưu khóa bảo mật.
  - Đóng gói dưới dạng **Flatpak** (Flathub) và AppImage.

### 3.2. Mobile Clients: iOS & Android (Phase 4+)
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
| **UI Clients** | UI Automation Tests | XCUITest (macOS) và WinAppDriver / Appium (Windows) cho các luồng chính: tạo note, checklist, khóa note và tìm kiếm theo [các mốc nghiệm thu](./03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu). |

---

## 5. Lộ trình Triển khai (Roadmap)

| Mốc | Phạm vi | Điều kiện hoàn thành |
| :--- | :--- | :--- |
| **Phase 1 — Prototype editor và FFI** | Dựng một editor thử trên mỗi hệ điều hành, với Rust core và cùng tài liệu mẫu. | Chạy bộ ca editor, ghi lại hệ điều hành, adapter, ca đạt/chưa đạt và lý do chọn adapter. Đo độ trễ gõ và áp patch trên cùng bộ dữ liệu. |
| **Phase 2 — Ghi chú offline trên macOS và Windows** | Hoàn thiện một luồng tạo, sửa, tổ chức và tìm lại ghi chú bằng sidebar, danh sách và editor. Có checklist, ảnh dán, liên kết ghi chú và tìm kiếm tiếng Việt. | Các luồng Phase 1–2 trong spec editor đạt trên cả hai client. Kill tiến trình sau xác nhận đã lưu và mở lại vẫn đọc được nội dung cùng ảnh đã lưu. |
| **Phase 3 — Đồng bộ E2EE hai thiết bị** | Dùng một bản self-host đơn node gồm Spring Boot, PostgreSQL và object storage tương thích S3. Client dùng polling, đồng bộ ngay khi quay lại app hoặc có mạng trở lại, và có nút **Đồng bộ ngay**. | Hai client hội tụ khi sửa offline. Kiểm tra retry, commit đồng thời, phân trang, polling, nút thủ công và các trigger trùng nhau. Đo từ lúc A xác nhận đã lưu đến lúc B hiển thị bản sửa khi B đang mở và khi B vừa quay lại app. Kiểm tra thêm thiết bị, khôi phục tài khoản và thu hồi quyền truy cập API của thiết bị. |
| **Phase 4+ — Tính năng và nền tảng mở rộng** | Locked Notes cục bộ, bảng đầy đủ, Smart Folders, Gallery, Quick Note, import/export và Linux/iOS/Android đi theo kết quả thử nghiệm. | Tính năng mới có ca nghiệm thu theo hành vi tham chiếu. Client mới đọc và sửa được tài liệu chung. Locked Notes phải đạt tiêu chí lưu trữ cục bộ và vòng đời khóa trước khi bật đồng bộ. |

Mỗi thử nghiệm ghi câu hỏi, dữ liệu mẫu, môi trường, tiêu chí đạt, kết quả và quyết định kỹ thuật. Với Phase 3, ghi riêng kết quả hội tụ của tài liệu và kết quả giữ các thay đổi độc lập. Compaction, HA, kiểm thử tải quy mô lớn và phân phối qua store có thể được triển khai theo nhu cầu của các mốc sau.
