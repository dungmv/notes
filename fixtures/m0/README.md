# Fixture editor M0 v1

[Tài liệu ban đầu](editor-v1.json) có tiếng Việt dựng sẵn và dấu tổ hợp, emoji 👩🏽‍💻, chữ đậm/nghiêng, hai cấp checklist và bảng 2 × 2. [Chuỗi thao tác](operations-v1.json) chạy mỗi mục ở một revision. Kết quả ở revision 8 là [snapshot đối chiếu](expected-v1.json). Nút **Replay fixture** chạy lại chuỗi từ revision 0 và hiển thị kết quả.

## Hợp đồng prototype

- JSON dùng UTF-8. Offset là số đơn vị UTF-16 trong nội dung một block hoặc một ô, bắt đầu từ 0. Range dùng dạng `[start, end)`. Core từ chối offset ngoài biên, range đảo và vị trí cắt cặp surrogate. Biên grapheme do editor native quản lý.
- Snapshot có `version`, `revision` và `blocks`. Gửi `{revision, operations, external}` theo revision đang hiển thị. Core áp tuần tự trong batch và hủy toàn bộ batch nếu có lỗi. Batch thay đổi trạng thái tăng revision một lần. Batch rỗng hoặc không đổi giữ nguyên revision.
- Update trả `base_revision`, `revision` và `patches`. Client kiểm tra `base_revision`, áp các lệnh xóa theo ID trước, sau đó cập nhật hoặc chèn block theo index tăng dần. Một patch bảng mang dữ liệu cả block, adapter xác định ô cần cập nhật. Lịch sử dùng cùng cơ chế patch. Batch `external` đứng ngoài lịch sử undo cục bộ.
- UniFFI Swift gọi `openFixture`, `openDocument`, `snapshot`, `apply`, `history` và API phiên bản `coreVersion`. C ABI Windows mở fixture bằng một handle sở hữu bởi adapter. Adapter đọc JSON trả về và giải phóng buffer bằng `notes_m0_string_free`.

## Môi trường và cách chạy

- macOS 14 trở lên với Xcode, Rust và bộ gõ tiếng Việt Telex. Chạy `bash scripts/build-macos-core.sh` từ repo, sau đó mở `clients/macos/Notes.xcodeproj` và chạy scheme **Notes**. Ghi model máy, phiên bản macOS/Xcode và trạng thái **TextKit 2** trên màn hình.
- Windows x64 hiện dùng .NET SDK 11, Windows App SDK 2.7.0, Rust MSVC và WinApp CLI. Chạy `./scripts/build-windows-core.ps1 -Run` từ repo. Trong Settings, thêm tiếng Việt và bàn phím Vietnamese Telex, rồi chọn qua `Win+Space` trước khi thử. Ghi phiên bản Windows, kiến trúc, Windows App SDK, adapter và binding.
- SQLite dùng `cargo run --locked --release -p notes-app --example m0_commit`. Mỗi lượt tạo DB SQLite mới dưới `target/m0-commit`, áp đúng tám thao tác.

## Ca bàn phím và kết quả mong đợi

| Ca | Thao tác và quan sát |
| --- | --- |
| Fixture | Nhấn **Replay fixture**. Kết quả là revision 8, cùng ID/thứ tự block và nội dung như snapshot đối chiếu. Mở lại app để bắt đầu từng ca ở revision 0. |
| IME tiếng Việt | Đặt con trỏ cuối đoạn đầu, chèn dấu cách rồi gõ Telex `tieengs Vieetj`. Trong khi chuỗi đang ghép, nhấn F6 để chèn `✓ ` ở block khác. Kiểm tra focus, caret và composition vẫn ở đoạn đầu. Thử commit bằng Space hoặc Return và ghi lại chữ cuối. Lặp lại với F7, kiểm tra marker được áp sau IME tại đầu vùng đang ghép. Lặp lại với Escape để kiểm tra hủy nếu bộ gõ hỗ trợ. Trên macOS có thể cần Fn để gửi F6/F7 thay cho phím chức năng hệ thống. Ghi riêng hành vi commit/hủy thực tế của từng bộ gõ. |
| Unicode và vùng chọn | Chọn emoji 👩🏽‍💻 bằng Shift+mũi tên, thay bằng 🙂, rồi chèn/xóa cạnh emoji. Kiểm tra vùng chọn và chuỗi trong UI, kể cả dấu tổ hợp ở mục con. Test Rust bao phủ thêm range cắt surrogate và ngoài biên. |
| Undo/redo | Sau một lần gõ, nhấn F6, rồi Ctrl+Z/Ctrl+Y trên Windows hoặc Cmd+Z/Cmd+Shift+Z trên macOS. So sánh trạng thái, caret và revision. Marker ở block khác phải giữ nguyên, mỗi thao tác phát đúng một lượt cập nhật. |
| Checklist | Trong mục đầu, thử Shift+Return để chèn dòng trong cùng mục. Return tách mục và chuyển focus sang mục mới. Tab/Shift+Tab đổi cấp. Bật checkbox và kiểm tra cập nhật giữ cấu trúc. |
| Bảng | Sửa ô hàng 2 cột 2 rồi nhấn F8 khi vẫn đang ở ô đó. Cùng hàng, ô bên cạnh nhận marker. Kiểm tra ô đang sửa giữ nội dung và caret, hàng/cột còn lại giữ nguyên. |
| Đo adapter | Ghi số mẫu, p50 và p95 hiển thị sau khi lặp ca. Đo từ callback bắt đầu sửa native đến khi áp xong cập nhật trong UI thread. Thời gian patch bắt đầu sau khi giải mã phản hồi FFI và kết thúc khi adapter đã ghi vào control hiện có. Ghi riêng mẫu replay, mẫu bàn phím và cấu hình máy. Các mốc này dừng trước frame được hệ điều hành trình bày. |

[Kế hoạch M0](../../docs/plans/20261127174136-m0-editor-ffi-prototype.md) · [Kết quả nghiệm thu](../../docs/m0-editor-prototype-results.md)
