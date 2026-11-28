# Kết quả prototype editor M0

**Trạng thái ngày 2026-11-28** — Windows M0.4 đạt các ca native. M0 còn mở để nghiệm thu macOS. [Kế hoạch](plans/20261127174136-m0-editor-ffi-prototype.md) · [Ca thử và lệnh chạy](../fixtures/m0/README.md)

## Kết quả theo ca

Windows chạy trên Windows 11 build 26300 x64, Intel Core i5-13500 (20 luồng logic), NTFS, .NET SDK 11.0.100 và Windows App SDK 2.7.0. Bản WinUI Debug dùng `RichEditBox` và Rust DLL Debug. Môi trường thực thi hiện tại là Windows. Cột macOS cần chạy trên macOS 14 trở lên với Xcode và Telex.

| Ca | Windows | macOS |
| --- | --- | --- |
| Mở và replay fixture | **Đạt.** WinApp UI Automation kích hoạt replay. Revision 8, Rust snapshot và text trong các control khớp fixture. Nội dung đoạn đầu là `Tiếng Việt 🙂 thử và á.`, ô R1 C1 là `Tên`, ô R2 C2 là `Hai`. [Ảnh kết quả](m0-windows-replay.png) | **Chưa chạy native.** Đã sinh binding Swift từ thư viện và đối chiếu chữ ký dùng trong adapter. |
| IME tiếng Việt, commit/hủy và patch | **Đạt với Vietnamese Telex tích hợp.** F6 cập nhật checklist và giữ preedit. F7 đợi composition kết thúc, sau đó chèn marker đúng vị trí và giữ caret sau dấu cách commit. Xóa preedit về rỗng hủy phần nhập mà giữ bản đã commit. | **Chưa chạy.** Cần xác minh trạng thái TextKit 2 và bộ gõ Telex. |
| Unicode và vùng chọn | **Đạt.** Từ fixture gốc, Home, 11 lần Right và Shift+Right chọn toàn bộ 👩🏽‍💻. Sau F6, vùng chọn vẫn giữ. Thay bằng 🙂 rồi Backspace chỉ xóa emoji. Rust kiểm tra ánh xạ hai chiều, surrogate và range ngoài biên. | **Chưa chạy native.** |
| Undo/redo và patch | **Đạt.** Trên bản đã sửa caret, từ revision 0 gõ `x`, nhấn F6, Ctrl+Z, Ctrl+Y rồi gõ `y`. Đoạn văn kết thúc bằng `xy` ở revision 5 và checklist giữ marker F6. Undo/redo không phát sinh mẫu nhập. | **Chưa chạy native.** |
| Checklist | **Đạt.** Shift+Return thêm dòng trong cùng mục, Return tách mục và chuyển caret, Tab/Shift+Tab đổi cấp và giữ focus. UI Automation đổi checkbox mục đầu từ off sang on. | **Chưa chạy native.** |
| Bảng | **Đạt.** Tại R2 C2, gõ `x`, nhấn F8 rồi gõ `y`. R2 C1 nhận marker, R2 C2 thành `Haixy`, hàng đầu giữ nguyên. | **Chưa chạy native.** |
| Commit/flush và retry | **Đạt trong benchmark SQLite độc lập.** Kiểm tra cả hai chiến lược ghi. | **Chưa đo trên macOS.** |

WinApp CLI mở app với package identity và gọi nút replay. Lượt nghiệm thu tiếp theo dùng Computer Use gửi phím vào control native, đối chiếu revision qua UI Automation và kiểm tra text, caret, vùng chọn và bố cục trên cửa sổ Notes.

`./scripts/build-windows-core.ps1` build thành công bản x64 với Rust DLL Release và WinUI Debug. Trong sandbox, build dùng cache NuGet cục bộ qua biến `RestoreSources` vì DNS của `api.nuget.org` bị chặn. Bảng số đo phân biệt lượt Rust DLL Debug và lượt Release.

### Bằng chứng composition trên Windows

Chạy `./scripts/build-windows-core.ps1 -Run` với Rust DLL Release, WinUI Debug, layout Vietnamese Telex tích hợp và UniKey ở chế độ E. Đặt caret cuối đoạn đầu của fixture gốc. Revision, trạng thái IME và nội dung được đọc từ cửa sổ Notes sau mỗi mốc.

| Mốc | Quan sát |
| --- | --- |
| Space, gõ `tieeng`, F6, gõ `s` | F6 tăng revision 1 → 2 và vẫn báo `IME composing`. Sau phím `s`, revision vẫn là 2. |
| Space | Commit `tiếng` và dấu cách ở revision 4. Checklist giữ marker F6. |
| Gõ `vieet`, F7, gõ `j` | Hiện `Patch queued until IME ends.` và revision giữ ở 4. |
| Space, gõ `tieengs` | Revision 7 sau commit và áp patch. Sau khi kết thúc từ kế tiếp, UI Automation đọc `Tiếng Việt 👩🏽‍💻 thử. tiếng ✓ việt tiếng`. Dấu cách sau `việt` được giữ và từ sau đứng đúng vị trí. |
| Gõ `viee`, Backspace ba lần | Preedit biến mất. Revision và nội dung đã commit giữ nguyên, không có mẫu nhập mới. |
| Gõ `viee`, F7, Backspace ba lần | Sau khi hủy, chỉ marker được commit. Revision 11 → 12, số mẫu nhập giữ ở 9. UI Automation đọc phần cuối `tiếng ✓ việt tiếng ✓ `. |

Escape có thể kết thúc preedit và giữ lại chữ trong Vietnamese Telex. Ở ca trên, Escape kết thúc từ `tiếng` và tăng một revision. Ca hủy được kiểm tra bằng Backspace về preedit rỗng.

UniKey trên layout US cũng cho `tiếng Việt` với F6 giữa từ. UniKey gửi các sửa đổi theo từng phím trong cấu hình này. Bản cuối áp F7 ngay khi gõ `tieeng`, rồi phím `s` tạo `tiêngs`. Giới hạn với bộ gõ ngoài Windows Telex đã được ghi nhận ở [TD-011](technical-debt.md).

## Rust và binding

- `cargo test --locked --workspace --all-features` đạt 4 test editor, gồm round-trip fixture, patch, tính nguyên tử khi lỗi range và lịch sử.
- Swift dùng UniFFI 0.33.0. Đã sinh `notes_ffi.swift` và kiểm tra các khai báo `openFixture`, `openDocument`, `snapshot`, `apply`, `history` và `coreVersion`. Cần Xcode để build, liên kết và chạy adapter.
- Windows đã thử `uniffi-bindgen-cs --library target/debug/notes_ffi.dll --crate notes_ffi --no-format --out-dir target/m0-csharp` bằng bản `v0.12.0+v0.32.0`. Generator dừng ở metadata constructor `OPEN_DOCUMENT` với lỗi `Unexpected metadata type code: 100`. Bản WinUI hiện dùng C ABI cùng lớp `PrototypeEditor` và đã replay thành công qua DLL. [Bản phát hành generator](https://github.com/NordSecurity/uniffi-bindgen-cs/releases/tag/v0.12.0%2Bv0.32.0)

## Số đo adapter

| Lượt Windows | Mẫu | p50 | p95 |
| --- | ---: | ---: | ---: |
| Áp patch từ replay v1, Rust DLL Debug | 8 | 1,742 ms | 8,774 ms |
| Sửa nội dung qua UniKey, checklist và bảng, Rust DLL Debug | 27 | 13,557 ms | 41,206 ms |
| Áp patch trong lượt UniKey, gồm replay và các phím thử, Rust DLL Debug | 43 | 4,676 ms | 6,074 ms |
| Sửa nội dung với Windows Vietnamese Telex, Rust DLL Release | 25 | 8,449 ms | 41,261 ms |
| Áp patch trong lượt Windows Vietnamese Telex, gồm replay, Rust DLL Release | 33 | 3,731 ms | 7,346 ms |

Lượt Telex riêng bắt đầu từ revision 8. Sau một dấu cách và 12 lần gõ `tieengs` + Space, revision là 33. UI Automation xác nhận đoạn văn khớp đầy đủ `Tiếng Việt 🙂 thử và á. ` theo sau bởi 12 lần `tiếng `. Mẫu sửa nội dung gồm 12 commit composition, 12 dấu cách kết thúc từ và dấu cách ban đầu. Đo từ callback bắt đầu sửa hoặc kết thúc composition đến khi UI thread áp xong cập nhật. Mẫu patch bắt đầu sau giải mã phản hồi C ABI. Số đo macOS còn chờ. [Ảnh WinUI](m0-windows-replay.png)

## Số đo SQLite

`cargo run --locked --release -p notes-app --example m0_commit` chạy bằng Rust 1.99.0 và SQLite 3.54.1 bundled trên cấu hình Windows ở trên. DB cục bộ mới cho mỗi lượt, WAL và `synchronous=FULL`. Tám thao tác v1 được lên lịch cách nhau 4 ms. Độ trễ tính từ mốc phát thao tác theo lịch, bao gồm xử lý Rust, thời gian đợi gom và ghi SQLite, đến lúc transaction commit thành công.

| Cách ghi | Lượt / mẫu | Kích thước batch và số commit | p50 | p95 |
| --- | --- | --- | ---: | ---: |
| Commit từng thao tác | 30 / 240 | 1 × 240 | 0,777 ms | 1,300 ms |
| Cửa sổ 12 ms | 30 / 240 | 1 × 10, 2 × 25, 3 × 60 | 8,786 ms | 19,160 ms |

Batch giảm từ 240 xuống 95 commit. Cuối chuỗi có flush ngay cho phần còn chờ. Benchmark kiểm tra lại snapshot và thứ tự journal, dừng tiến trình con ngay sau ACK rồi mở lại DB, và chèn lỗi SQLite ở giữa batch để kiểm tra rollback, giữ batch chưa ACK và retry theo thứ tự. Cả hai chiến lược phục hồi đúng snapshot v1 sau khi dừng tiến trình.

## Lựa chọn cho M1 và M2

- M1 dùng **commit từng thao tác đã hoàn tất** làm mặc định dựa trên p95 hiện tại. Duy trì API nhận batch nguyên tử và ACK sau commit. Đo lại với tài liệu dài, burst nhập và ổ đĩa trên cả hai OS trước khi đổi mặc định. Benchmark đang lưu snapshot mẫu và operation journal bằng một kết nối, tách khỏi vòng đời app native.
- Chọn WinUI `RichEditBox` với C ABI cho Windows theo kết quả M0.4. Giữ `NSTextView` với TextKit 2 và UniFFI Swift làm ứng viên macOS cho đến khi chạy đủ ca native.
- M2 cần đánh giá lại hình chiếu bảng, lựa chọn binding C# và hành vi lịch sử khi external patch sửa cùng block. Phần triển khai M0 dùng control cho từng block hoặc ô và undo theo snapshot của block.

[Sổ nợ kỹ thuật](technical-debt.md)
