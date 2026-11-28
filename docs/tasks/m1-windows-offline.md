# M1 Windows · MVP offline

**Phụ thuộc** — [M0.4](./m0-4-windows-editor-prototype.md), [M1.1](./m1-1-local-storage-and-migrations.md), [M1.2](./m1-2-persistent-crdt-editor.md), [M1.3](./m1-3-vietnamese-search.md) và [kế hoạch Windows](../plans/20261128102214-m1-windows-offline.md).

**Phạm vi** — Rust core offline và ứng dụng Windows native với giao diện ba cột, vòng đời thư mục và ghi chú, editor, tìm kiếm tiếng Việt, checklist, ảnh và liên kết nội bộ.

## Tiêu chí nghiệm thu

- [x] Rust core lưu ghi chú, revision, chỉ mục và metadata trong transaction. Mở lại đọc được dữ liệu đã ACK. Lỗi ghi rollback và cho phép thử lại.
- [x] App WinUI build và mở được. Bài tự kiểm thử chạy trong WinUI xác minh soạn thảo, tìm kiếm, checklist, ảnh, liên kết theo ID và vòng đời ghi chú qua dữ liệu cục bộ.
- [x] Kiểm tra migration, hồi quy fixture M0 và phục hồi sau dừng tiến trình qua Rust/C ABI. Ghi lại bằng chứng cùng các ca còn chờ kiểm chứng.
- [x] Chạy app và kiểm tra dữ liệu cục bộ khi máy thử nghiệm ngắt mạng.
- [x] Chạy phím tắt, Vietnamese Telex và dán ảnh từ clipboard hệ thống qua UI native.

## Hợp đồng dữ liệu và lưu

- SQLite schema 3. Migration 1 → 2 tạo và dựng lại FTS5. Migration 2 → 3 thêm tùy chọn sắp xếp checklist. Tất cả thao tác migration của một lần mở nằm trong cùng transaction. Ghi chú xóa mềm giữ ID và thư mục gốc. Khôi phục về **Ghi chú** khi thư mục gốc mất hoặc đã xóa.
- Tài liệu sử dụng Yrs update v1. Chữ và định dạng nằm trong các kiểu Text của Yrs. Map `document` chứa JSON snapshot v1 gồm thứ tự block, loại block và revision. Offset trao đổi qua FFI tính bằng UTF-16. Ảnh và liên kết được đặt vào block giữa phần trước và sau caret.
- C ABI `notes_offline_open`, `notes_offline_call` và `notes_offline_free` truyền yêu cầu JSON UTF-8 qua handle. Phản hồi chứa `value` hoặc `error`. Buffer được giải phóng bằng `notes_m0_string_free`. `apply`, `history` và `paste_image` trả revision và patch sau commit. Khi lỗi ghi, phiên giữ revision trước đó và client có thể gửi lại batch. WinUI giữ thay đổi chưa ACK trong phiên và chặn đổi ghi chú hoặc thoát bình thường cho đến khi lưu được.
- FTS5 gộp các từ truy vấn theo AND và tìm theo tiền tố. Truy vấn chỉ có dấu câu không tạo kết quả. Tìm trong ghi chú so khớp chuỗi con và quy đổi về range UTF-16 gốc. Cả hai chuẩn hóa dấu tiếng Việt dựng sẵn, dấu tổ hợp và `đ`/`d`.
- Ảnh được ghi vào tệp tạm, đồng bộ và công bố vào vị trí SHA-256 trước khi SQLite commit tham chiếu. Blob đã ghi nhưng chưa được tham chiếu có thể được dùng lại khi retry. Metadata cũ được giữ để redo có thể trỏ lại ảnh.

## Kết quả ngày 2026-11-28

- Windows 11 build 26300 x64, .NET SDK 11.0.100, Windows App SDK 2.7.0 và Rust 1.99.0. Build DLL Release và WinUI Debug qua [script Windows](../../scripts/build-windows-core.ps1), mở được gói debug với DB thử nghiệm riêng.
- WinUI Release build được với `-p:PublishReadyToRun=false`. Lượt build dùng cache của cấu hình ReadyToRun mặc định thiếu gói runtime để tối ưu assembly.
- Mười test Rust/C ABI đạt, gồm bốn hồi quy M0, vòng đời và tìm kiếm SQLite, migration, rollback/retry, định dạng và checklist, ảnh/link và tiến trình con bị dừng sau ACK. Clippy đạt với `-D warnings`. Bài đo Rust Release trên ghi chú 130.000 byte qua 40 lần ghi cho p50 33,93 ms và p95 121,67 ms, tính đến commit SQLite.
- WinUI native tạo thư mục và ghi chú qua UI. Chế độ `-VerifyM1` chạy control thật để xác minh Unicode, ACK, định dạng, undo/redo, retry khi kích hoạt lỗi giả lập, checklist, ảnh từ Windows `DataPackage`, xóa/khôi phục và cả hai dạng nhãn liên kết. Ca ghi thất bại qua transaction SQLite thật được kiểm chứng trong Rust.

## Nghiệm thu trên Windows sạch ngày 2026-11-28

- Windows Sandbox x64 build 26100 chạy bản WinUI Debug self-contained với `<Networking>Disable</Networking>`. Guest có 0 card mạng đang hoạt động và 0 tuyến mặc định IPv4/IPv6. Sáu nhóm `VerifyM1` đều đạt, gồm lưu, tìm kiếm, checklist, mở lại ảnh, liên kết và xóa/khôi phục. Báo cáo và log nằm tại `target/m1-disconnected/offline-acceptance.json` và `offline-static.log`. [Cấu hình Windows Sandbox](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file)
- Lượt đầu trên guest sạch mở được cửa sổ nhưng không nạp được `notes_ffi.dll`. `dumpbin /DEPENDENTS` xác nhận DLL nhập `VCRUNTIME140.dll`. Script build Windows giờ liên kết tĩnh CRT cho Rust và SQLite. Bản mới hết phụ thuộc này và hoàn tất cả sáu nhóm trên cùng guest khi mạng vẫn tắt. Build WinUI và 10 test Rust/C ABI cũng đạt.

| Ca native | Kết quả trên guest |
| --- | --- |
| Phím tắt | Ctrl+N tạo ghi chú và đưa focus vào RichEditBox. Ctrl+B/I/U định dạng vùng chọn, Ctrl+Z/Y hoàn tác và làm lại. Ctrl+F và Ctrl+Shift+F đưa focus vào đúng ô tìm kiếm. Ctrl+K và `>>` đều mở hộp chọn liên kết. App đặt focus ban đầu vào ô tìm kiếm sau khi mở dữ liệu. Xem `target/m1-disconnected/keyboard-clipboard.log`. |
| Vietnamese Telex | Text Services Framework xác nhận profile Vietnamese Telex đang hoạt động. Gửi từng phím `tieengs Vieetj` kèm dấu cách cho ra `tiếng Việt`, nhận **Đã lưu** và tìm lại bằng `tieng viet`. Lượt trước khi kích hoạt profile cho ra chuỗi phím thô. Xem `target/m1-disconnected/telex-input.log`. [Danh sách profile Windows](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/default-input-locales-for-windows-language-packs) |
| Clipboard hệ thống | Đặt bitmap mẫu vào clipboard của guest, đặt caret giữa `Before` và `After`, rồi nhấn Ctrl+V. Ảnh hiện đúng vị trí sau khi thoát và mở lại. Lặp lại với Telex đang bật và caret giữa `123` và `456`. SQLite ghi một tham chiếu ảnh, hash trên đĩa khớp metadata và `PRAGMA integrity_check` trả `ok`. Xem `target/m1-disconnected/clipboard-native.log` và `clipboard-reopen-telex.png`. |

[Fixture, lệnh chạy và ma trận ca native](../../fixtures/m1/README.md) · [Sổ nợ kỹ thuật](../technical-debt.md)

[Bàn giao sang M2 Windows](../m2-windows-acceptance.md) đối chiếu các ca M1 với kết quả ở ứng viên M2.
