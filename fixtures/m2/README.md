# Fixture desktop v1 và ma trận M2

Fixture gồm thư mục **Công việc**, ghi chú `desktop-v1` từ tài liệu [M0](../m0/editor-v1.json), ảnh PNG [M1](../m1/pixel.png) và ghi chú đích **Đích fixture**. Chương trình tạo DB chèn ảnh giữa `Tiếng Việt` và phần còn lại của đoạn đầu rồi thêm hai liên kết cùng ID đích. Một liên kết lấy nhãn từ tiêu đề và liên kết còn lại dùng **Nhãn riêng**. Tài liệu giữ bảng 2 × 2, đánh dấu đậm và nghiêng, emoji nhiều mã Unicode và hai mục checklist khác cấp/trạng thái.

## Chuẩn bị và chạy trên Windows

```powershell
cargo build --locked -p notes-app --example desktop_fixture
./scripts/build-windows-core.ps1 -StandaloneOutput target/m2/app
$result = Join-Path target ('m2-' + (Get-Date -Format yyyyMMddHHmmss))
./scripts/verify-windows-m2.ps1 -App target/m2/app/Notes.exe -FixtureTool target/debug/examples/desktop_fixture.exe -Results $result
```

Để xác nhận offline trên x64, build công cụ fixture với CRT liên kết tĩnh trước khi chuyển vào Windows Sandbox sạch.

```powershell
cargo build --locked --release -p notes-app --example desktop_fixture --target x86_64-pc-windows-msvc --config "target.x86_64-pc-windows-msvc.rustflags = ['-C', 'target-feature=+crt-static']"
```

Chuyển bản WinUI self-contained, `target/x86_64-pc-windows-msvc/release/examples/desktop_fixture.exe` và script nghiệm thu sang Sandbox với `<Networking>Disable</Networking>`. Chép chúng vào ổ đĩa guest, dùng đường dẫn riêng trong guest cho `-Results`, rồi thêm `-RequireOffline`. Lưu lại JSON và stdout. Chạy với thư mục kết quả mới cho mỗi lượt.

`desktop_fixture prepare <thư-mục-mới>` tạo DB schema 3, blob và `expected-v1.json`. `prepare-v1` tạo cùng nội dung theo cấu trúc schema 1, gồm tham chiếu và blob, để kiểm tra migration thật khi mở app. ID sinh trong lúc chuẩn bị khác nhau giữa các lượt. Khi đối chiếu hai client, dùng cùng một bản sao fixture và JSON của lần tạo đó.

## Chuyển dữ liệu giữa hai hệ điều hành

1. Tạo fixture schema 3 một lần rồi lưu bản gốc. Sao chép **toàn bộ** thư mục cho lượt Windows → macOS và lượt macOS → Windows.
2. A mở bản sao, sửa một ô bảng từ **Một** thành **Hai 😀** và đợi ACK. Đóng phiên và thoát tất cả tiến trình dùng thư mục đó. Chuyển cả cây thư mục, gồm DB, tệp phụ SQLite nếu còn và `blobs/`. Giữ nguyên bản sao nguồn cho đến khi B mở thành công.
3. B xác nhận ô đã sửa, in đậm và nghiêng gốc, vị trí emoji, thứ tự/cấp/trạng thái checklist, ba ô bảng còn lại, vị trí/bytes ảnh và hai link cùng ID. Sửa một ô khác, đợi ACK, đóng phiên và chuyển toàn bộ thư mục về A theo cùng quy trình. A xác nhận cả hai sửa đổi và đối chiếu các phần còn lại.
4. Lưu thứ tự hai client, phiên bản app và fixture, snapshot trước và sau, SHA-256 của blob và kết quả từng bước. Ảnh gốc có SHA-256 `42ff80f86ca28d848679ebfe6118a085c5d1ea9270ee3075be205e0d22bf673b`.

## Ma trận ca thử

| Ca | Kết quả mong đợi | Đường chạy |
| --- | --- | --- |
| Điều hướng, phím tắt và FTS tiếng Việt | Tạo, đổi tên thư mục, đổi ghi chú, di chuyển, xóa, khôi phục đúng ID. FTS có dấu, không dấu và tổ hợp trả cùng ID. Tìm trong ghi chú chọn đúng range gốc. | Windows dùng Ctrl+N, Ctrl+F, Ctrl+Shift+F. macOS dùng Cmd+N, Cmd+F, Cmd+Opt+F. Theo [fixture M1](../m1/README.md#ca-windows-native). |
| FFI, SQLite và tái mở | Lỗi UTF-16 không đổi revision. Đọc lại qua handle mới giữ định dạng. Dữ liệu schema 1 nâng lên schema 3 và tìm được qua FTS. | `verify` trên Windows. Dùng cùng snapshot và thao tác qua Swift khi có adapter offline. |
| Editor, IME và bảng | Sửa cạnh emoji, kiểm tra caret và vùng chọn, sửa một ô bảng và giữ các ô khác. Khi patch đến giữa composition, hoàn tất hoặc hủy IME đúng; undo và redo theo revision. | `verify` kiểm bảng và xung đột cùng block. Bản Debug Windows có `--verify-ime-patches` cho F6/F7/F8 trong phiên offline. Đối chiếu Telex và UniKey riêng. macOS chạy các ca IME theo [M0.6](../../docs/tasks/m0-6-editor-prototype-acceptance.md). |
| Checklist | Enter tách, Shift+Enter xuống dòng, Tab/Shift+Tab đổi cấp. Tick/bỏ tick với tự sắp rồi mở lại kiểm trạng thái và cấp. | Ctrl+Shift+L hoặc Cmd+Shift+L tạo/tick checklist. So sánh với checklist v1 và thực hiện các phím trên từng client. |
| Ảnh và link | Ctrl+V hoặc Cmd+V đặt bitmap giữa đoạn. `>>` và Ctrl+K hoặc Cmd+K tạo link theo đúng ID. Đổi tên đích chỉ đổi nhãn theo tiêu đề. | Kiểm tra clipboard hệ thống và bàn phím theo M1. `image` dùng Windows DataPackage và đường lưu thật. |
| Lỗi ghi, đổi ghi chú và thoát | Trigger SQLite chặn ghi văn bản hoặc ảnh. UI không báo **Đã lưu**, chặn rời ghi chú, cho lưu lại cùng revision và chỉ thêm một ảnh. | `retry` cài trigger vào DB thử nghiệm, gỡ từng trigger sau khi app báo lỗi rồi thử lại trong chính phiên. |
| Dừng sau ACK | Hai lượt riêng cho sửa văn bản và dán ảnh. Dừng tiến trình sau khi có ACK, mở lại đối chiếu snapshot, FTS, vị trí ảnh, hash và cả hai link. | `text` và `image` dừng chính tiến trình WinUI, sau đó mở app lại trên cùng dữ liệu. |
| Hiệu năng | Lưu mẫu thô, số mẫu, p50/p95, bản build và môi trường. | Tám tiến trình mới từ yêu cầu khởi chạy đến lần sửa bằng RichEditBox có focus. 40 lần chèn một ký tự vào tài liệu 130.000 byte. Ghi thời gian gọi C ABI đến ACK và từ `TextChanging` tới khi WinUI áp patch xong. |

Trên Windows, kích hoạt profile **Vietnamese Telex** trước khi mở app với `--verify-ime-patches` và thư mục dữ liệu thử nghiệm mới. Tạo ghi chú, gõ `tieeng`, nhấn F7, gõ `s` rồi Space. Sau ACK, editor có `✓ tiếng `. Gõ tiếp `viee`, nhấn F7 rồi Backspace ba lần để hủy preedit. Sau ACK, editor giữ `✓ tiếng ✓ `. Cả hai lần nhấn F7 trong lúc gõ phải báo đang chờ composition kết thúc.

[Kế hoạch M2](../../docs/plans/20261128095337-m2-desktop-offline.md) · [Biên bản M2 Windows](../../docs/m2-windows-acceptance.md)
