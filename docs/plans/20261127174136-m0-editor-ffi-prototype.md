# Kế hoạch M0 · Prototype editor và FFI

Mục tiêu là chọn được adapter soạn thảo cho macOS và Windows, xác nhận cả hai dùng chung hợp đồng Rust và chọn cách gom batch dựa trên số đo commit. M0 dùng tài liệu mẫu và một đường ghi SQLite tối thiểu để đo. Luồng quản lý ghi chú, tìm kiếm, đính kèm, đồng bộ và mã hóa thuộc các mốc sau. [Roadmap M0](../roadmap.md#m0--kiểm-chứng-editor-và-ffi)

## Các đầu việc

| ID | Phụ thuộc | Công việc | Đầu ra kiểm chứng được |
| --- | --- | --- | --- |
| [M0.1](../tasks/m0-1-shared-fixtures.md) | — | Chuẩn bị tài liệu và ca thử chung | Fixture dùng chung có đoạn tiếng Việt, emoji ghép, định dạng inline, checklist nhiều cấp và bảng đơn giản. Có thao tác mẫu, trạng thái tài liệu mong đợi và hướng dẫn chạy ca IME bằng bàn phím. Xác định môi trường chạy macOS và Windows. |
| [M0.2](../tasks/m0-2-rust-editor-ffi.md) | M0.1 | Xây hợp đồng editor Rust và FFI | Dùng một phần mô hình CRDT đủ cho fixture. Mở tài liệu, áp batch thao tác và trả patch tăng dần theo block. Hợp đồng xác định đơn vị offset, revision và thứ tự cập nhật. Test Rust kiểm tra round-trip, range không hợp lệ và patch trên fixture. Duy trì API phiên bản đang dùng. |
| [M0.3](../tasks/m0-3-macos-editor-prototype.md) | M0.2 | Dựng prototype macOS | Bọc `NSTextView` qua SwiftUI, gọi Rust qua UniFFI, áp patch gia tăng và xác minh các luồng thử chạy bằng TextKit 2. Ghi kết quả theo fixture và đo độ trễ adapter. |
| [M0.4](../tasks/m0-4-windows-editor-prototype.md) | M0.2 | Dựng prototype Windows | Nối WinUI 3 `RichEditBox` với cùng hợp đồng Rust. Thử UniFFI C# trước và kiểm tra khả năng dùng C ABI nếu binding không đáp ứng. Nếu `RichEditBox` hụt ca bắt buộc, thử WebView2 cục bộ trên các ca đó và đo lại. Có bằng chứng chạy trên Windows cho adapter được chọn. |
| [M0.5](../tasks/m0-5-sqlite-commit-benchmark.md) | M0.2 | Đo chi phí transaction và batch | Dùng SQLite file cục bộ với WAL và `synchronous=FULL`. So sánh ghi từng thao tác với batch trong khoảng ngắn trên cùng chuỗi thao tác. Đo từ lúc thao tác đến xác nhận sau commit và kiểm tra flush, thứ tự, lỗi ghi và retry. |
| [M0.6](../tasks/m0-6-editor-prototype-acceptance.md) | M0.3, M0.4, M0.5 | Nghiệm thu và chốt lựa chọn | Công bố kết quả từng ca trên từng OS, số đo, adapter và binding đã chọn, chính sách batch, giới hạn còn lại và ảnh hưởng đến M1/M2. |

M0.3–M0.5 có thể tiến hành độc lập sau khi hợp đồng dùng chung đã ổn định. M0.4 cần môi trường Windows có thể chạy WinUI 3 và xác minh IME. Nếu môi trường đó chưa sẵn sàng, hoàn thành các đầu việc độc lập và ghi rõ ca Windows còn chờ xác minh.

## Bộ ca nghiệm thu

| Ca | Kết quả cần quan sát trên cả hai nền tảng |
| --- | --- |
| Nhập tiếng Việt và áp patch | Gõ tiếng Việt bằng IME, kiểm tra chuỗi đang ghép và thao tác commit/hủy. Patch ở block khác giữ composition và vùng chọn hiện tại. Khi patch đụng vùng đang ghép, ghi lại cách adapter xử lý và kiểm tra nội dung cuối. |
| Unicode và vùng chọn | Chèn, xóa và thay thế cạnh emoji ghép và ký tự có dấu. Kiểm tra ánh xạ UTF-16 sang vị trí Rust/CRDT và chiều ngược lại. Range ngoài biên hoặc cắt cặp surrogate trả lỗi rõ ràng. |
| Undo/redo và patch | Undo/redo cập nhật nhất quán cả editor và tài liệu Rust. Patch từ core cập nhật đúng block, giữ vùng chọn theo thao tác và tránh phát lại patch thành thao tác nhập mới. |
| Checklist | Return tạo mục mới, Shift+Return xuống dòng cùng mục, Tab/Shift+Tab đổi cấp. Bật/tắt mục cập nhật trạng thái trong Rust và giữ cấu trúc danh sách khi áp patch. |
| Bảng cơ bản | Render bảng từ fixture, sửa nội dung một ô và áp patch ở một ô khác. Hàng, cột và dữ liệu các ô còn lại giữ nguyên. |
| Commit và retry | Xác nhận lưu đến sau commit. Dừng tiến trình sau xác nhận rồi đọc lại tài liệu mẫu. Lỗi ghi để lại trạng thái đã commit trước đó và batch còn chờ có thể thử lại theo thứ tự. |

## Đo lường và điều kiện đóng M0

- Dùng cùng phiên bản fixture và chuỗi thao tác để đo độ trễ từ thao tác đến khi editor phản hồi, thời gian áp patch và thời gian đến lúc nhận xác nhận commit. Ghi số lượt chạy, p50, p95, cấu hình máy, hệ điều hành, adapter, binding và kích thước batch. So sánh các phương án trên cùng máy.
- Lưu cách tái hiện, kết quả Đạt/Không đạt/Chưa chạy và bằng chứng cho từng ca. Các ca bắt buộc cần đạt trên cả hai nền tảng trước khi chốt M0. Kết quả build và test Rust không thay thế phép thử native.
- Ghi quyết định adapter, binding và batch kèm kết quả đo trong tài liệu nghiệm thu. Cập nhật [sổ nợ kỹ thuật](../technical-debt.md) với các giới hạn, đánh đổi và phép thử còn thiếu. Chỉ chuyển `TD-001` sang `Resolved` sau khi có đủ kết quả Windows và số đo batch.

Tham chiếu [kiến trúc FFI](../specs/01-architecture-and-ffi.md), [hợp đồng lưu trữ](../specs/02-data-model-and-storage.md), [đặc tả editor](../specs/03-features-and-editor.md) và [nghiệm thu client](../specs/05-platform-clients-and-roadmap.md).

[Fixture và hướng dẫn tái hiện](../../fixtures/m0/README.md) · [Kết quả nghiệm thu đang mở](../m0-editor-prototype-results.md)
