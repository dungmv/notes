# Notes

Monorepo cho ứng dụng ghi chú. Xem kiến trúc và lộ trình trong [đặc tả dự án](docs/specs/README.md).

| Thành phần | Thư mục | Công nghệ |
| --- | --- | --- |
| Ứng dụng macOS | `clients/macos/` | SwiftUI, Xcode, macOS 14+ |
| Core dùng chung | `core/` | Rust, Cargo workspace, UniFFI |
| Backend | `backend/` | Java 21, Spring Boot 4.1.3, Maven Wrapper |

## macOS

Cần Xcode 15.3 trở lên và Rust stable. Bản Rust do Homebrew cung cấp có thể kèm thư viện chuẩn được build cho macOS mới hơn, gây cảnh báo khi liên kết cho macOS 14. [Cài Rust qua rustup](https://rustup.rs/).

Chạy các lệnh sau từ root repo.

```sh
./scripts/build-macos-core.sh
open clients/macos/Notes.xcodeproj
```

Chọn scheme **Notes** và nhấn **⌘R**. Cửa sổ hiển thị phiên bản core do Rust trả về qua UniFFI. Script tạo Swift bindings và XCFramework cho kiến trúc của máy đang build. Chạy lại script sau khi sửa Rust hoặc đổi kiến trúc build.

Có thể build và mở ứng dụng từ terminal.

```sh
xcodebuild -project clients/macos/Notes.xcodeproj -scheme Notes \
  -destination "platform=macOS,arch=$(uname -m)" -derivedDataPath target/xcode build
open target/xcode/Build/Products/Debug/Notes.app
```

## Rust

Kiểm tra Cargo workspace từ root repo.

```sh
cargo test --workspace --locked
```

`notes-app` cung cấp API lõi. `notes-ffi` xuất API sang Swift qua UniFFI.

## Backend

Cần JDK 21 trở lên. Maven Wrapper tự tải Maven ở lần chạy đầu.

```sh
cd backend
./mvnw spring-boot:run
```

Kiểm tra backend từ terminal khác.

```sh
curl -fsS http://localhost:8080/actuator/health
```

Phản hồi phải có trường `"status":"UP"`. Backend khởi động độc lập cho mốc khởi tạo, trước khi thêm lưu trữ và đồng bộ theo đặc tả.
