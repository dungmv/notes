# Roadmap phát triển Notes

*Cập nhật ngày 27/11/2026.*

**MVP** là bản macOS dùng được hằng ngày khi offline. Người dùng có thể tạo, sửa, sắp xếp và tìm lại ghi chú tiếng Việt, với checklist, ảnh và liên kết giữa các ghi chú. **Desktop 1.0** hoàn thiện trải nghiệm trên macOS và Windows, đồng bộ mã hóa đầu cuối (E2EE), khóa ghi chú cục bộ và có quy trình nâng cấp, sao lưu, khôi phục đã được thử nghiệm.

Repo hiện có khung ứng dụng macOS gọi Rust qua UniFFI và khung Spring Boot với endpoint health. Lưu trữ ghi chú, editor, client Windows và đồng bộ là các mốc tiếp theo. [Hướng dẫn chạy dự án](../readme.md)

## Các mốc phát hành

| Mốc | Kết quả người dùng nhận được | Điều kiện chuyển mốc |
| --- | --- | --- |
| M0 · Prototype (Phase 1) | Editor thử nghiệm trên macOS và Windows dùng chung tài liệu Rust | Chốt adapter từng nền tảng dựa trên kết quả kiểm thử thao tác và đo độ trễ. |
| M1 · MVP macOS (Phase 2) | Ghi chú offline, tìm kiếm tiếng Việt, checklist, ảnh và liên kết ghi chú | Toàn bộ luồng MVP chạy trên macOS. Ghi chú và ảnh đã báo lưu còn nguyên sau khi dừng tiến trình và mở lại. |
| M2 · Desktop offline (Phase 2) | Windows có cùng các luồng offline với macOS | Cả hai client đạt cùng bộ ca nghiệm thu và đọc, sửa được tài liệu mẫu dùng chung. |
| M3 · Beta đồng bộ E2EE (Phase 3) | Hai thiết bị đồng bộ qua backend self-host | Hai client hội tụ sau khi sửa offline, giữ được các thay đổi độc lập và vượt qua các ca thử phục hồi, thu hồi phiên và toàn vẹn dữ liệu. |
| M4 · Desktop 1.0 (Phase 4) | Bản desktop ổn định, có khóa ghi chú cục bộ, tính năng tổ chức nâng cao và import/export | Hai client đạt ca nghiệm thu desktop. Bản cài, nâng cấp dữ liệu và khôi phục từ bản sao lưu đã được kiểm chứng. |
| M5 · Mở rộng nền tảng (Phase 4+) | Linux, iOS/iPadOS hoặc Android theo nhu cầu đã xác định | Mỗi client đạt bộ ca tương thích tài liệu, đồng bộ và quản lý khóa trước khi phát hành. |

Thứ tự phát hành là macOS offline, Windows offline, beta đồng bộ rồi Desktop 1.0. Prototype Windows bắt đầu từ M0 để kiểm chứng định dạng tài liệu và hợp đồng FFI trước khi mở rộng editor macOS. Roadmap dùng tiêu chí nghiệm thu để xác định thời điểm phát hành. Công việc triển khai chi tiết cho từng mốc được lưu trong [docs/plans](./plans/) theo [quy ước dự án](../AGENTS.md). [Lộ trình trong đặc tả client](./specs/05-platform-clients-and-roadmap.md#5-lộ-trình-triển-khai-roadmap)

## M0 · Kiểm chứng editor và FFI

- Dựng editor thử nghiệm trên macOS và Windows với cùng tài liệu rich text và hợp đồng thao tác qua Rust. Thử gõ tiếng Việt bằng IME, sửa cạnh emoji, giữ vùng chọn, undo/redo và áp patch.
- Ghi lại hệ điều hành, tài liệu mẫu, ca đạt, ca lỗi và độ trễ gõ, áp patch, commit. Chốt adapter Windows và cách gom batch commit dựa trên kết quả (`TD-001`). [Kiến trúc FFI](./specs/01-architecture-and-ffi.md) · [Nghiệm thu editor](./specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu)

## M1 · MVP macOS offline

[Kế hoạch triển khai và danh sách task M1](./plans/20261128101843-m1-macos-offline.md)

- Hoàn thiện luồng từ Rust core đến SwiftUI/AppKit. Lưu thư mục, ghi chú và tài liệu rich text CRDT trong SQLite, lưu ảnh trong blob store và dùng FTS5 để tìm kiếm cục bộ. Trạng thái **Đã lưu** xuất hiện sau khi core xác nhận commit.
- Giao diện ba cột hỗ trợ tạo, sửa, di chuyển, xóa và khôi phục ghi chú. Editor có định dạng cơ bản, checklist, dán ảnh tại con trỏ và liên kết ghi chú bằng `>>`. Người dùng có thể tìm trong ghi chú hoặc tìm toàn bộ bằng tiếng Việt không dấu.
- Nghiệm thu trên macOS với mạng tắt. Kiểm tra mở lại sau khi dừng tiến trình ngay sau xác nhận lưu, lỗi ghi và thử lại, migration SQLite, phím tắt, checklist và nhãn liên kết khi đổi tên ghi chú đích. [Lưu trữ](./specs/02-data-model-and-storage.md) · [Tính năng editor](./specs/03-features-and-editor.md)

## M2 · Windows và bộ kiểm thử desktop chung

[Kế hoạch triển khai và danh sách task M2](./plans/20261128095337-m2-desktop-offline.md)

- Xây dựng client WinUI 3 bằng adapter đã chốt ở M0. Dùng lại Rust core, định dạng tài liệu, logic tìm kiếm và lưu trữ. Áp dụng quy ước bàn phím và điều hướng của Windows.
- Chạy cùng bộ dữ liệu và ca nghiệm thu trên macOS và Windows. Kiểm tra hợp đồng Swift/C# qua FFI, mở lại sau khi dừng tiến trình, phần soạn thảo rich text và tương thích tài liệu mẫu theo cả hai chiều.
- Ghi số đo cold start tới khi có thể gõ, độ trễ gõ và thời gian từ thao tác đến xác nhận lưu. Lưu kèm cấu hình thiết bị, phiên bản hệ điều hành và bộ dữ liệu để so sánh giữa các bản. [Đặc tả client và kiểm thử](./specs/05-platform-clients-and-roadmap.md)

## M3 · Beta đồng bộ E2EE

- Triển khai bản self-host một node bằng Spring Boot, PostgreSQL và MinIO hoặc kho tương thích S3. Mã hóa delta và ảnh tại client. Khi bật đồng bộ lần đầu, đưa các ghi chú đủ điều kiện đã tạo offline vào outbox.
- Kiểm chứng retry sau mất phản hồi, push đồng thời, pull phân trang và tiến trình bị dừng giữa chừng. Đo độ trễ giữa hai thiết bị khi app đang mở hoặc vừa mở lại, rồi dùng số liệu để điều chỉnh polling (`TD-004`). Kiểm tra nút **Đồng bộ ngay** và các lần kích hoạt đồng bộ trùng nhau.
- Kiểm tra riêng hai kết quả sau khi hai máy sửa offline. Cả hai phải thấy cùng nội dung, định dạng, checklist và ảnh. Các chỉnh sửa độc lập ở những vùng khác nhau phải được giữ.
- Nghiệm thu thêm thiết bị, đổi mật khẩu, phục hồi tài khoản (`TD-005`) và thu hồi quyền truy cập API của thiết bị. Chạy bộ kiểm thử mật mã trong CI và kiểm tra dữ liệu máy chủ, log và backup theo ranh giới tin cậy. [Giao thức đồng bộ](./specs/04-sync-and-security.md) · [Backend](./specs/06-backend-spring-boot.md) · [Kiểm thử E2EE](./specs/07-e2ee-cryptography.md#10-kiểm-thử--tiêu-chuẩn-nghiệm-thu-bảo-mật-security-verification)

## M4 · Desktop 1.0

[Kế hoạch triển khai và danh sách task M4 — phần Windows](./plans/20261128183931-m4-desktop-1.0-windows.md)

- Hoàn thiện Locked Notes cục bộ với Keychain/Touch ID và Windows Hello theo khả năng thiết bị. Kiểm tra ghi chú được tạo trước khi khóa, dữ liệu dẫn xuất trong FTS, thumbnail, tag và trạng thái sau khi khóa lại hoặc khởi động lại. Đồng bộ Locked Notes là một mốc riêng sau khi kiểm chứng đổi khóa, lịch sử dữ liệu và thiết bị offline (`TD-003`).
- Hoàn thiện bảng, tag, Smart Folders, Gallery, Quick Note và import/export. Dùng bộ dữ liệu mẫu để xác định và kiểm thử trường được giữ khi import/export, gồm nội dung, định dạng và ảnh theo phạm vi từng định dạng.
- Tạo bản cài macOS và Windows. Với macOS, kiểm chứng XCFramework trên Apple Silicon và Intel (`TD-006`). Thử cài mới và nâng cấp từ bản trước, kiểm tra phím tắt, focus bàn phím và trình đọc màn hình ở các luồng chính.
- Diễn tập sao lưu và khôi phục PostgreSQL cùng object storage, rồi kiểm tra client đồng bộ tiếp sau khi phục hồi. Dựa vào số liệu beta để quyết định ưu tiên snapshot/compaction (`TD-002`), kể cả thử thiết bị offline lâu và retry qua ranh giới snapshot khi có thay đổi. [Spec editor](./specs/03-features-and-editor.md) · [Vận hành backend](./specs/06-backend-spring-boot.md#5-vận-hành-bảo-mật--mở-rộng)

## M5 · Sau Desktop 1.0

- Xếp thứ tự Linux, iOS/iPadOS và Android theo nhu cầu sử dụng. Mỗi client cần mở, sửa và đồng bộ tài liệu chung qua Rust core, xử lý offline đúng và tích hợp kho khóa của hệ điều hành.
- Xem xét phân phối qua store, kiểm thử tải lớn và HA dựa trên nhu cầu phát hành và số liệu self-host. Theo dõi các hạng mục, trạng thái và bằng chứng xử lý trong [sổ nợ kỹ thuật](./technical-debt.md). [Định hướng nền tảng](./specs/05-platform-clients-and-roadmap.md#3-kế-hoạch-mở-rộng-tương-lai)
