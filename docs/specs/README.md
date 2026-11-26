# Đặc tả Kỹ thuật: Cross-Platform Notes App (Apple Notes Clone)

Tài liệu này là bộ đặc tả (specifications) cho dự án xây dựng ứng dụng ghi chú đa nền tảng mang trải nghiệm tương tự **Apple Notes**, sử dụng **Rust** làm lõi nghiệp vụ dùng chung (`notes-core`) và giao diện **Native UI** cho từng hệ điều hành.

---

## 1. Tầm nhìn & Mục tiêu

- **Trải nghiệm Native thuần túy**: Không dùng Electron hay cross-platform UI nặng nề; mỗi nền tảng dùng UI toolkit chính chủ để đạt hiệu năng, độ mượt, phím tắt, tích hợp hệ thống (Touch ID, Windows Hello, kéo thả, Quick Note) tốt nhất.
- **Single Source of Truth bằng Rust**: Toàn bộ logic nghiệp vụ, mô hình dữ liệu, Rich-Text CRDT, tìm kiếm toàn văn (FTS), mã hóa đầu cuối (E2EE), lưu trữ cục bộ (SQLite + Blob Store) và động cơ đồng bộ (Sync Engine) nằm trong `notes-core`.
- **Offline-first & Real-time Sync**: Ghi chú hoạt động tức thì khi mất mạng, tự động gộp xung đột (conflict-free) khi đồng bộ giữa nhiều thiết bị nhờ CRDT.
- **Mở rộng dài hạn**: Khởi đầu với **macOS (SwiftUI)** và **Windows (WinUI 3)**, kiến trúc sẵn sàng mở rộng cho **Linux** và **Mobile (iOS, Android)**.

---

## 2. Nền tảng mục tiêu & Công nghệ

| Giai đoạn | Nền tảng | UI Framework | Giao tiếp với Rust Core |
| :--- | :--- | :--- | :--- |
| **Phase 1–2** | **macOS** | SwiftUI (+ AppKit cho Editor/Windowing nâng cao) | UniFFI (Swift bindings + XCFramework) |
| **Phase 1–2** | **Windows** | WinUI 3 (Windows App SDK, C#) | UniFFI C# (`uniffi-bindgen-cs`) hoặc `csbindgen` (C-ABI) |
| **Phase 3** | **Linux** | GTK4 + libadwaita (Rust hoặc C++/Vala) | Gọi trực tiếp crate `notes-core` (nếu dùng `gtk4-rs`) hoặc C-ABI |
| **Phase 3** | **Backend Sync** | Java 21+ / Spring Boot 4.x + PostgreSQL + S3/MinIO + Centrifugo | HTTPS REST + Centrifugo (`notes-sync`) |
| **Phase 4** | **iOS / iPadOS** | SwiftUI (+ UIKit bridges) | Dùng chung UniFFI Swift Package với macOS |
| **Phase 4** | **Android** | Jetpack Compose (Kotlin) | UniFFI Kotlin bindings (JNI `.so`) |

---

## 3. Danh mục Tài liệu Specs

0. [00-system-architecture.md](./00-system-architecture.md) — Kiến trúc tổng quan toàn hệ thống (End-to-End), luồng dữ liệu, ranh giới tin cậy và mô hình triển khai.
1. [01-architecture-and-ffi.md](./01-architecture-and-ffi.md) — Kiến trúc bên trong Rust Core, cấu trúc Workspace, FFI Boundary, State Management & Threading.
2. [02-data-model-and-storage.md](./02-data-model-and-storage.md) — Mô hình dữ liệu (Folder, Note, Tag, Attachment), CRDT Document Model, SQLite + FTS5 & Blob Storage.
3. [03-features-and-editor.md](./03-features-and-editor.md) — Đặc tả tính năng giống Apple Notes, thiết kế Rich Text Editor trên macOS/Windows, Locked Notes, Smart Folders.
4. [04-sync-and-security.md](./04-sync-and-security.md) — Giao thức đồng bộ Offline-first, Mã hóa đầu cuối (E2EE), Key Management & Biometrics.
5. [05-platform-clients-and-roadmap.md](./05-platform-clients-and-roadmap.md) — Đặc tả chi tiết cho macOS, WinUI 3, định hướng Linux/Mobile, chiến lược kiểm thử và lộ trình phát triển.
6. [06-backend-spring-boot.md](./06-backend-spring-boot.md) — Đặc tả Backend Service sử dụng Java / Spring Boot, PostgreSQL và S3-compatible Storage (MinIO/S3).

---

## 4. Nguyên tắc thiết kế (Design Principles)

1. **Thin UI, Fat Core**: Lớp UI chỉ giữ trạng thái hiển thị (view state), xử lý layout, animation và native input. Mọi validate, phân loại Smart Folder, parse `#tag`, tính toán sort order, lưu trữ và sync đều thuộc về `notes-core`.
2. **Idempotent & Deterministic**: Mọi thao tác chỉnh sửa nội dung ghi chú đi qua CRDT operations để đảm bảo đồng bộ đa thiết bị không bao giờ mất dữ liệu.
3. **Zero-Knowledge Security**: Dữ liệu nhạy cảm (Locked Notes và tùy chọn E2EE toàn bộ tài khoản) được mã hóa tại client trước khi ghi xuống sync payload; server không bao giờ nắm khóa giải mã.
4. **Hiệu năng khởi động < 300ms**: `notes-core` mở SQLite ở chế độ WAL, lazy-load nội dung ghi chú và blob đính kèm theo nhu cầu.
