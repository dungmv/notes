# Kết quả prototype editor M0

**Trạng thái ngày 2026-11-28** — Đã có fixture, hợp đồng Rust, hai adapter và benchmark SQLite. M0 còn mở để nghiệm thu native. [Kế hoạch](plans/20261127174136-m0-editor-ffi-prototype.md) · [Ca thử và lệnh chạy](../fixtures/m0/README.md)

## Kết quả theo ca

Windows chạy trên Windows 11 build 26300 x64, Intel Core i5-13500 (20 luồng logic), NTFS, .NET SDK 11.0.100 và Windows App SDK 2.7.0. Bản WinUI Debug dùng `RichEditBox` và Rust DLL Debug. Môi trường thực thi hiện tại là Windows. Cột macOS cần chạy trên macOS 14 trở lên với Xcode và Telex.

| Ca | Windows | macOS |
| --- | --- | --- |
| Mở và replay fixture | **Đạt.** WinApp UI Automation kích hoạt replay. Revision 8, Rust snapshot và text trong các control khớp fixture. Nội dung đoạn đầu là `Tiếng Việt 🙂 thử và á.`, ô R1 C1 là `Tên`, ô R2 C2 là `Hai`. [Ảnh kết quả](m0-windows-replay.png) | **Chưa chạy native.** Đã sinh binding Swift từ thư viện và đối chiếu chữ ký dùng trong adapter. |
| IME tiếng Việt, commit/hủy và patch | **Chưa chạy.** Phiên desktop chỉ có layout US `0409:00000409`. Cần Telex để xác minh F6 giữ composition và F7 áp marker sau composition. | **Chưa chạy.** Cần xác minh trạng thái TextKit 2 và bộ gõ Telex. |
| Unicode và vùng chọn | **Đạt phần replay Unicode.** Rust kiểm tra ánh xạ hai chiều, surrogate và range ngoài biên. Chọn, xóa và thay trực tiếp bằng phím còn chờ một phiên desktop điều khiển được. | **Chưa chạy native.** |
| Undo/redo và patch | **Chưa xác minh bằng bàn phím.** Rust đạt ca undo/redo sau external patch ở block khác và ca undo/redo tách mục. | **Chưa chạy native.** |
| Checklist | **Đạt phần replay và toggle.** UI Automation đổi checkbox mục đầu từ off sang on. Replay render đúng mục mới, hai cấp lồng và dòng phụ. Cần xác minh riêng Return, Shift+Return và Tab/Shift+Tab. | **Chưa chạy native.** |
| Bảng | **Đạt phần replay.** Hai ô ở hai vị trí cập nhật theo thứ tự, bảng giữ đủ bốn ô. Ca bàn phím sửa ô rồi F8 và kiểm tra caret còn chờ xác minh. | **Chưa chạy native.** |
| Commit/flush và retry | **Đạt trong benchmark SQLite độc lập.** Kiểm tra cả hai chiến lược ghi. | **Chưa đo trên macOS.** |

WinApp CLI mở app với package identity, đọc được cây UI Automation và gọi nút replay. Việc gửi phím thiếu foreground hoặc bị computer-use ngắt khi phát hiện thao tác đồng thời. Một lần gửi chuỗi phím cho ô R2 C2 cho thấy giá trị `Hai` nhưng marker ở ô bên cạnh chưa quan sát được, nên giữ ca F8 ở trạng thái chưa xác minh.

`./scripts/build-windows-core.ps1` build thành công bản x64 với Rust DLL Release và WinUI Debug. Trong sandbox, build dùng cache NuGet cục bộ qua biến `RestoreSources` vì DNS của `api.nuget.org` bị chặn. Phép đo UI ở dưới dùng bản Rust DLL Debug.

## Rust và binding

- `cargo test --locked --workspace --all-features` đạt 4 test editor, gồm round-trip fixture, patch, tính nguyên tử khi lỗi range và lịch sử.
- Swift dùng UniFFI 0.33.0. Đã sinh `notes_ffi.swift` và kiểm tra các khai báo `openFixture`, `openDocument`, `snapshot`, `apply`, `history` và `coreVersion`. Cần Xcode để build, liên kết và chạy adapter.
- Windows đã thử `uniffi-bindgen-cs --library target/debug/notes_ffi.dll --crate notes_ffi --no-format --out-dir target/m0-csharp` bằng bản `v0.12.0+v0.32.0`. Generator dừng ở metadata constructor `OPEN_DOCUMENT` với lỗi `Unexpected metadata type code: 100`. Bản WinUI hiện dùng C ABI cùng lớp `PrototypeEditor` và đã replay thành công qua DLL. [Bản phát hành generator](https://github.com/NordSecurity/uniffi-bindgen-cs/releases/tag/v0.12.0%2Bv0.32.0)

## Số đo adapter

Một lượt replay v1 trên WinUI gồm 8 thao tác, gửi lần lượt qua C ABI. Áp patch trên UI thread có **p50 1,742 ms, p95 8,774 ms** với 8 mẫu. Các số trên đo đoạn cập nhật control sau khi giải mã patch. Mẫu độ trễ gõ và kết quả IME của Windows, cùng toàn bộ số đo native macOS, còn chờ xác minh. [Ảnh WinUI](m0-windows-replay.png)

## Số đo SQLite

`cargo run --locked --release -p notes-app --example m0_commit` chạy bằng Rust 1.99.0 và SQLite 3.54.1 bundled trên cấu hình Windows ở trên. DB cục bộ mới cho mỗi lượt, WAL và `synchronous=FULL`. Tám thao tác v1 được lên lịch cách nhau 4 ms. Độ trễ tính từ mốc phát thao tác theo lịch, bao gồm xử lý Rust, thời gian đợi gom và ghi SQLite, đến lúc transaction commit thành công.

| Cách ghi | Lượt / mẫu | Kích thước batch và số commit | p50 | p95 |
| --- | --- | --- | ---: | ---: |
| Commit từng thao tác | 30 / 240 | 1 × 240 | 0,777 ms | 1,300 ms |
| Cửa sổ 12 ms | 30 / 240 | 1 × 10, 2 × 25, 3 × 60 | 8,786 ms | 19,160 ms |

Batch giảm từ 240 xuống 95 commit. Cuối chuỗi có flush ngay cho phần còn chờ. Benchmark kiểm tra lại snapshot và thứ tự journal, dừng tiến trình con ngay sau ACK rồi mở lại DB, và chèn lỗi SQLite ở giữa batch để kiểm tra rollback, giữ batch chưa ACK và retry theo thứ tự. Cả hai chiến lược phục hồi đúng snapshot v1 sau khi dừng tiến trình.

## Lựa chọn cho M1 và M2

- M1 dùng **commit từng thao tác đã hoàn tất** làm mặc định dựa trên p95 hiện tại. Duy trì API nhận batch nguyên tử và ACK sau commit. Đo lại với tài liệu dài, burst nhập và ổ đĩa trên cả hai OS trước khi đổi mặc định. Benchmark đang lưu snapshot mẫu và operation journal bằng một kết nối, tách khỏi vòng đời app native.
- Giữ `NSTextView` với TextKit 2 và UniFFI Swift, cùng WinUI `RichEditBox` với C ABI, làm các ứng viên M1. Chốt sau khi chạy đủ ca native. Thử WebView2 cục bộ trên các ca bắt buộc mà RichEditBox được xác nhận có lỗi.
- M2 cần đánh giá lại hình chiếu bảng, lựa chọn binding C# và hành vi lịch sử khi external patch sửa cùng block. Phần triển khai M0 dùng control cho từng block hoặc ô và undo theo snapshot của block.

[Sổ nợ kỹ thuật](technical-debt.md)
