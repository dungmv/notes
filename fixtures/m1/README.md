# Fixture offline Windows M1

## Cách chạy

```powershell
cargo test --locked --offline --workspace --features notes-ffi/c-editor
cargo test --locked --offline --release -p notes-app --test offline profile_long_note -- --ignored --nocapture
./scripts/build-windows-core.ps1 -Run -DataDirectory target/m1-native
$testData = Join-Path target ('m1-winui-' + (Get-Date -Format yyyyMMddHHmmss))
./scripts/build-windows-core.ps1 -Run -VerifyM1 -DataDirectory $testData
```

Dùng một thư mục dữ liệu mới cho mỗi lượt nghiệm thu. `reopen_after_terminating_writer` tạo tiến trình con, chờ ACK revision 2 từ C ABI, dừng tiến trình và mở lại DB để kiểm tra văn bản, ảnh và FTS. `v1_migrates_from_m0_document_and_rolls_back_on_sql_error` dựng DB schema 1 từ [fixture M0](../m0/editor-v1.json) và kiểm tra migration đến schema 3.

Tùy chọn `-VerifyM1` mở bản WinUI Debug, chạy qua các control và adapter đang dùng trong app rồi ghi `winui-acceptance.json` trong thư mục thử nghiệm. Ảnh lấy từ một `DataPackage` của Windows nên lượt tự động tránh ghi đè clipboard hệ thống.

[pixel.png](pixel.png) là PNG RGBA 16 × 16, SHA-256 `42ff80f86ca28d848679ebfe6118a085c5d1ea9270ee3075be205e0d22bf673b`. Test C ABI chèn ảnh sau chữ `Tiếng` vào ghi chú `Tiếng Việt 😀` và kiểm tra ảnh nằm giữa hai block văn bản, đúng hash và bytes. Khi dán qua WinUI, bitmap được mã hóa lại thành PNG và hash tính từ bytes đầu ra.

## Ca Windows native

| Ca | Thao tác và kết quả mong đợi |
| --- | --- |
| Điều hướng | Tạo và đổi tên thư mục, tạo ghi chú, di chuyển, xóa và khôi phục. ID giữ nguyên. Ghi chú bị xóa chỉ xuất hiện trong **Đã xóa gần đây**. |
| Tìm kiếm | Dùng nội dung `Đề xuất Tiếng Việt 😀`. Tìm `de xuat` và `đề xuất` ở danh sách toàn bộ. Dùng Ctrl+F chọn `tieng viet` trong editor. Ctrl+Shift+F đưa focus vào ô tìm toàn bộ. |
| Editor | Gõ bằng Vietnamese Telex, sửa cạnh emoji, chọn văn bản và dùng Ctrl+B/I/U, tạo tiêu đề, đổi ghi chú rồi mở lại. Ctrl+Z/Y điều khiển lịch sử của phiên. |
| Checklist | Gõ `[] ` ở đoạn trống. Enter tạo mục tiếp, Shift+Enter xuống dòng, Tab đổi cấp. Bật **Tự sắp checklist** rồi tick và bỏ tick. Mục đã tick xuất hiện sau các mục chưa tick trong cùng nhóm. |
| Ảnh | Sao chép ảnh từ trình xem ảnh, dán giữa `Tiếng` và ` Việt 😀`, rồi đổi ghi chú và mở lại. Kiểm tra vị trí và ảnh hiển thị. |
| Liên kết | Tạo ghi chú đích và nguồn. Gõ `>>`, lọc và chọn đích bằng mũi tên và Enter. Dùng Ctrl+K tạo thêm một link có nhãn riêng. Đổi tên, di chuyển, xóa và khôi phục ghi chú đích. Mở lại nguồn để kiểm tra nhãn và điều hướng. |
| Lưu và khôi phục | Chờ **Đã lưu**, dừng tiến trình, mở lại cùng thư mục dữ liệu. Kiểm tra nội dung và ảnh. Dùng trigger SQLite trong test Rust để kiểm tra rollback và retry theo revision. |

[Task và kết quả nghiệm thu M1 Windows](../../docs/tasks/m1-windows-offline.md)
