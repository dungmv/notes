# Biên bản M2 Windows · 2026-11-28

[Kế hoạch M2](./plans/20261128095337-m2-desktop-offline.md) · [Biên bản M1 Windows](./tasks/m1-windows-offline.md) · [Fixture desktop v1 và lệnh chạy](../fixtures/m2/README.md)

Ứng viên Windows x64 đã qua bộ tự kiểm WinUI cho bảng, FFI, migration, khôi phục sau dừng tiến trình và retry lỗi SQLite thật trên host và Windows Sandbox tắt mạng. M2 tiếp tục mở cho Swift offline, nghiệm thu macOS và tương thích hai chiều.

## Đối chiếu và kết quả

M1 đã nghiệm thu WinUI Debug self-contained x64 trong Windows Sandbox build 26100 với mạng tắt. Các lượt native có bằng chứng cho phím tắt tìm kiếm và rich text, Vietnamese Telex, clipboard bitmap hệ thống và mở lại ghi chú. M1 thử dừng sau ACK qua tiến trình con C ABI, và thử lỗi ghi ở WinUI bằng lỗi giả lập. M2 bổ sung kiểm chứng trên chính tiến trình WinUI và transaction SQLite.

| Task | Kết quả Windows và phần còn mở |
| --- | --- |
| [M2.1](./tasks/m2-1-windows-build-and-offline-ffi.md) | Windows App SDK 2.7.0, SDK BuildTools 10.0.28000.2957, .NET 11.0.100 và Rust 1.99.0. Lệnh `-StandaloneOutput` dựng WinUI Debug cùng DLL Release liên kết tĩnh CRT. Phiên mở SQLite trên worker rồi trả về dispatcher, nơi các lời gọi sau đó được tuần tự hóa. `LibraryImport` sinh marshalling UTF-8, SafeHandle sở hữu phiên, mỗi buffer phản hồi được Rust giải phóng. C ABI phản hồi đồng bộ, nên bước mở bất đồng bộ dùng kết quả qua dispatcher. Timer refresh hủy đăng ký khi đóng. CI restore sạch và các kiến trúc x86/ARM64 còn chờ. |
| [M2.2](./tasks/m2-2-windows-navigation-and-search.md) | Dùng lại luồng ba cột và phím tắt M1. M2 kiểm FTS không dấu, dấu tổ hợp sau khi mở lại và cập nhật FTS khi retry commit. Ma trận vẫn giữ các bước chọn bằng chuột và bàn phím, đổi tên và di chuyển qua dialog để chạy native đầy đủ. |
| [M2.3](./tasks/m2-3-windows-editor.md) | Kiểm tra sửa/định dạng một ô bảng qua RichEditBox và giữ ba ô bên cạnh. Hoàn tác báo xung đột khi patch chạm cùng block, giữ bản vá và cho tiếp tục sửa. C ABI giữ trường `error` và thêm `code: "history_conflict"` cho ca này. Khôi phục vùng chọn UTF-16 trong phiên khi quay lại ghi chú. Ca checklist dùng phím Ctrl+Shift+L, Enter, Shift+Enter và Tab/Shift+Tab đạt trong Sandbox. Shift+Enter lưu LF trong cùng mục, như fixture chung. Vietnamese Telex tích hợp đạt cả commit và hủy với F7 trên control đang gõ. UniKey còn mở theo TD-011. |
| [M2.4](./tasks/m2-4-windows-attachments-and-links.md) | Dùng bằng chứng Ctrl+K và clipboard hệ thống ở M1. M2 bổ sung chọn đích bằng mũi tên và Enter từ `>>` rồi mở link đúng ghi chú trong Sandbox. Lỗi insert metadata ảnh thật và thử lại, vị trí ảnh, SHA-256 và cả hai dạng nhãn trong fixture đều đạt. |
| [M2.5](./tasks/m2-5-desktop-fixtures-and-compatibility.md) | Có fixture và quy trình chuyển trọn DB/blob. Kiểm C# gọi DLL Rust thật và từ chối range tách surrogate trong 8 vòng mở/đóng. Còn kiểm hợp đồng Swift offline và hai chiều Windows ↔ macOS. |
| [M2.6](./tasks/m2-6-desktop-durability-and-recovery.md) | Hai lượt WinUI độc lập dừng tiến trình sau ACK văn bản hoặc ảnh. Mở lại đối chiếu revision, ảnh, link, bảng, checklist và FTS. Trigger SQLite từ chối ghi văn bản và ghi tham chiếu ảnh. UI giữ revision, chặn rời ghi chú và thử lại trong phiên. Migration fixture schema 1 giữ nội dung, link và blob, dựng lại FTS và cho ghi tiếp. Còn lượt macOS. |
| [M2.7](./tasks/m2-7-desktop-performance-and-acceptance.md) | Có đo thời gian tại RichEditBox và qua C ABI với tài liệu dài. M0.6, M1.8 và các lượt thử hai nền tảng vẫn là điều kiện đóng M2. |

## Cách đo

- Khởi động đo từ lúc runner yêu cầu tạo tiến trình đến sự kiện `TextChanging` của lần sửa đầu trên editor đã focus. Mỗi mẫu tạo tiến trình và DB riêng, với bộ đệm hệ điều hành có thể đã nóng. Dữ liệu gõ được đưa vào control bằng bài thử.
- Độ trễ WinUI đo từ `TextChanging` đến lúc áp patch xong. Độ trễ ACK đo từ lúc gọi C ABI đến lúc nhận và giải mã phản hồi sau commit. Cả hai dùng cùng 40 lần chèn một ký tự vào ghi chú có đoạn 130.000 byte. p50/p95 dùng thứ hạng làm tròn lên.
- M1 đo Rust đến SQLite commit trên tài liệu 130.000 byte cho p50 33,93 ms và p95 121,67 ms. M2 gồm chi phí FFI và chuyển đổi JSON trong phép đo ACK. Cần số đo input vật lý, commit IME và frame hiển thị trước khi kết luận ngưỡng phản hồi trên hai nền tảng.
- M0 đo từ callback chỉnh sửa hoặc kết thúc composition tới khi áp patch trên fixture ngắn, với p50 8,449 ms và p95 41,261 ms qua 25 mẫu Telex. So sánh M0 và M2 cần tính đến chiều dài tài liệu, cách tạo input và đường lưu bền vững được thêm trong M1. [Số đo M0](./m0-editor-prototype-results.md)

## Số đo Windows x64

| Môi trường | Phép đo | Mẫu | p50 (ms) | p95 (ms) |
| --- | --- | ---: | ---: | ---: |
| Host Windows 11 build 26300 | Khởi chạy đến lần sửa đầu | 8 | 814,09 | 1.123,75 |
| Host Windows 11 build 26300 | C ABI đến ACK | 40 | 9,17 | 15,03 |
| Host Windows 11 build 26300 | RichEditBox đến khi áp patch xong | 40 | 238,54 | 271,15 |
| Sandbox Windows 11 build 26100 | Khởi chạy đến lần sửa đầu | 8 | 832,07 | 942,47 |
| Sandbox Windows 11 build 26100 | C ABI đến ACK | 40 | 7,46 | 70,94 |
| Sandbox Windows 11 build 26100 | RichEditBox đến khi áp patch xong | 40 | 239,96 | 311,11 |

WinUI Debug self-contained và Rust DLL Release, fixture desktop v1. SHA-256 `Notes.dll` là `5c4920c6a561a1fa3335f3c045bae6c9642fb7194cb376d244c27b5fd715b287`. SHA-256 `notes_ffi.dll` là `674fd2078de0587a6e6813d73766ea1cf9ed5d627595a17885390759d27cf8a7`. Mẫu thô và snapshot host nằm tại `target/m2/host-m2/`. Mười test Rust/C ABI đạt và Clippy đạt với `-D warnings`.

Máy dùng Intel Core i5-13500, ổ NTFS. Sandbox có 4 GiB RAM, 0 adapter hoạt động và 0 tuyến mặc định. Log và báo cáo M1/M2 nằm tại `target/m2/offline-m2/`. `native.log` ghi ca checklist và chọn đích từ `>>`. Lượt Telex ban đầu mất focus sau khi đổi bộ gõ. `telex-only.log` ghi lượt đạt khi cấu hình profile trước lúc mở app. Build WinUI Release x64 cũng đạt với `-p:PublishReadyToRun=false`.

UniKey 4.6.2.0 còn chờ xác nhận. Lượt đầu trong Sandbox nhận nguyên chuỗi `tieengs` ở bước đối chứng. Lượt thử bật chế độ gõ lại bị mất focus nên chưa đến bước F7. Hai lượt được ghi trong `unikey.log` và `unikey-retry.log`. Giữ kết quả M0 và TD-011 để đối chiếu khi có phiên UniKey hoạt động ổn định.

p95 của luồng RichEditBox trên tài liệu dài là 271,15 ms trên host và 311,11 ms trong Sandbox. Cần đo riêng chi phí xử lý WinUI và lưu theo TD-009, TD-014 trước khi chốt chính sách commit.

[Nợ kỹ thuật](./technical-debt.md) · [Nghiệm thu macOS M1.8](./tasks/m1-8-macos-offline-acceptance.md)
