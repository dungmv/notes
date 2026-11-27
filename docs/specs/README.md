# Đặc tả Kỹ thuật: Ứng dụng ghi chú đa nền tảng

Tài liệu này là bộ đặc tả (specifications) cho dự án xây dựng ứng dụng ghi chú đa nền tảng mang trải nghiệm tương tự **Apple Notes**, sử dụng **Rust** làm lõi nghiệp vụ dùng chung (`notes-core`) và giao diện **Native UI** cho từng hệ điều hành.

---

## 1. Tầm nhìn & Mục tiêu

- **Trải nghiệm tham chiếu**. Apple Notes trên macOS là chuẩn hành vi cho các luồng ghi, tổ chức và tìm lại ghi chú. Client dùng UI native và quy ước của hệ điều hành. Các hành vi chung và ngoại lệ được mô tả trong [spec editor](./03-features-and-editor.md).
- **Nghiên cứu công nghệ**. Dự án dùng prototype và ca kiểm thử để đánh giá native editor qua FFI, hội tụ và bảo toàn ý định khi sửa offline, cùng vòng đời khóa E2EE. Tiến độ được đánh giá theo kết quả kiểm chứng và chất lượng trải nghiệm. Phân phối qua store và mở rộng hạ tầng theo sau nhu cầu sử dụng hoặc câu hỏi nghiên cứu cụ thể.
- **Single Source of Truth bằng Rust**: Toàn bộ logic nghiệp vụ, mô hình dữ liệu, Rich-Text CRDT, tìm kiếm toàn văn (FTS), mã hóa đầu cuối (E2EE), lưu trữ cục bộ (SQLite + Blob Store) và động cơ đồng bộ (Sync Engine) nằm trong `notes-core`.
- **Offline-first và sync đa thiết bị**. Có thể ghi và tìm kiếm khi mất mạng. Thay đổi đã xác nhận lưu tồn tại sau khi khởi động lại. Thiết bị hội tụ khi kết nối lại theo [hợp đồng sync](./04-sync-and-security.md).
- **Mở rộng dài hạn**: Khởi đầu với **macOS (SwiftUI)** và **Windows (WinUI 3)**, kiến trúc sẵn sàng mở rộng cho **Linux** và **Mobile (iOS, Android)**.

---

## 2. Nền tảng mục tiêu & Công nghệ

| Giai đoạn | Nền tảng | UI Framework | Giao tiếp với Rust Core |
| :--- | :--- | :--- | :--- |
| **Phase 1–2** | **macOS** | SwiftUI (+ AppKit cho Editor/Windowing nâng cao) | UniFFI (Swift bindings + XCFramework) |
| **Phase 1–2** | **Windows** | WinUI 3 (Windows App SDK, C#) | UniFFI C# (`uniffi-bindgen-cs`) hoặc `csbindgen` (C-ABI) |
| **Phase 4+** | **Linux** | GTK4 + libadwaita (Rust hoặc C++/Vala) | Gọi trực tiếp crate `notes-core` (nếu dùng `gtk4-rs`) hoặc C-ABI |
| **Phase 3** | **Backend Sync** | Java 21+ / Spring Boot 4.x + PostgreSQL + S3/MinIO | HTTPS REST do `notes-sync` khởi tạo qua polling hoặc nút **Đồng bộ ngay** |
| **Phase 4+** | **iOS / iPadOS** | SwiftUI (+ UIKit bridges) | Dùng chung UniFFI Swift Package với macOS |
| **Phase 4+** | **Android** | Jetpack Compose (Kotlin) | UniFFI Kotlin bindings (JNI `.so`) |

---

## 3. Danh mục Tài liệu Specs

0. [00-system-architecture.md](./00-system-architecture.md) — Kiến trúc tổng quan toàn hệ thống (End-to-End), luồng dữ liệu, ranh giới tin cậy và mô hình triển khai.
1. [01-architecture-and-ffi.md](./01-architecture-and-ffi.md) — Kiến trúc bên trong Rust Core, cấu trúc Workspace, FFI Boundary, State Management & Threading.
2. [02-data-model-and-storage.md](./02-data-model-and-storage.md) — Mô hình dữ liệu (Folder, Note, Tag, Attachment), CRDT Document Model, SQLite + FTS5 & Blob Storage.
3. [03-features-and-editor.md](./03-features-and-editor.md) — Đặc tả tính năng giống Apple Notes, thiết kế Rich Text Editor trên macOS/Windows, Locked Notes, Smart Folders.
4. [04-sync-and-security.md](./04-sync-and-security.md) — Giao thức đồng bộ Offline-first, Mã hóa đầu cuối (E2EE), Key Management & Biometrics.
5. [05-platform-clients-and-roadmap.md](./05-platform-clients-and-roadmap.md) — Đặc tả chi tiết cho macOS, WinUI 3, định hướng Linux/Mobile, chiến lược kiểm thử và lộ trình phát triển.
6. [06-backend-spring-boot.md](./06-backend-spring-boot.md) — Đặc tả Backend Service sử dụng Java / Spring Boot, PostgreSQL và S3-compatible Storage (MinIO/S3).
7. [07-e2ee-cryptography.md](./07-e2ee-cryptography.md) — Đặc tả chi tiết Mã hóa Đầu cuối (E2EE): Threat Model, Cipher Suite, Phân cấp Khóa, Wire Format & AAD, Streaming Blobs, Zero-Knowledge Auth và Vòng đời Khóa.

---

## 4. Nguyên tắc thiết kế (Design Principles)

1. **Thin UI, Fat Core**: Lớp UI chỉ giữ trạng thái hiển thị (view state), xử lý layout, animation và native input. Mọi validate, phân loại Smart Folder, parse `#tag`, tính toán sort order, lưu trữ và sync đều thuộc về `notes-core`.
2. **Idempotent & Deterministic**: Thao tác chỉnh sửa nội dung đi qua CRDT. Kiểm thử riêng khả năng hội tụ, lưu bền vững và giữ ý định người dùng.
3. **Zero-Knowledge Security**: Mọi nội dung đồng bộ được mã hóa ở client. Locked Notes có lớp bảo vệ độc lập theo [chính sách lưu trữ](./02-data-model-and-storage.md#34-chính-sách-lưu-trữ-locked-notes).
4. **Hiệu năng đo trên từng client**. Đo cold start tới khi có thể gõ, độ trễ gõ và độ trễ từ thao tác tới xác nhận lưu. Báo cáo thiết bị, hệ điều hành và bộ dữ liệu để so sánh kết quả prototype.
