# Technical Debt Register

Tài liệu theo dõi và kiểm soát toàn bộ nợ kỹ thuật (technical debt) trong dự án. Mọi giải pháp tạm thời, đánh đổi thiết kế, hoặc phần cần refactor đều phải được ghi vào đây theo quy định tại [AGENTS.md](../AGENTS.md).

---

## Active Technical Debt

| ID | Khu vực / File | Mô tả & Lý do | Mức độ | Hướng xử lý đề xuất | Trạng thái | Ngày ghi nhận |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| TD-001 | Editor, FFI và SQLite ([kết quả M0](./m0-editor-prototype-results.md)) | Đã nghiệm thu RichEditBox với Telex tích hợp và đo commit SQLite trên fixture. macOS còn chờ build, thử native và đo. | High | Chạy đủ ca TextKit 2 trên macOS để chốt M0. Đo lại chính sách commit trên tài liệu thực. | In Progress | 2026-11-27 |
| TD-002 | Sync backend ([06](./specs/06-backend-spring-boot.md)) | Ghi delta tuần tự theo tài khoản để bảo đảm cursor. Log Phase 3 tăng dần vì giữ lịch sử cho retry và thiết bị offline lâu. | Medium | Đo tranh chấp khóa và chi phí bootstrap. Thiết kế compaction có hợp đồng snapshot, phục hồi và giữ biên nhận. | Open | 2026-11-27 |
| TD-003 | Locked Notes ([02](./specs/02-data-model-and-storage.md), [07](./specs/07-e2ee-cryptography.md)) | Đồng bộ Locked Notes hoãn vì bản ghi và blob cũ có thể vẫn mở được bằng khóa của ghi chú thường. | High | Kiểm thử thay khóa, chuyển dữ liệu cục bộ cũ, xử lý log/snapshot trước khi khóa và đồng bộ với thiết bị offline. | Open | 2026-11-27 |
| TD-004 | Sync transport ([04](./specs/04-sync-and-security.md), [06](./specs/06-backend-spring-boot.md)) | Client polling qua HTTPS giúp giảm hạ tầng. Độ trễ nhận bản sửa phụ thuộc chu kỳ poll và có request ngay cả khi ít thay đổi. | Low | Đo số lượt pull rỗng và độ trễ nhận thay đổi khi cả hai máy đang mở hoặc vừa quay lại app. Điều chỉnh chu kỳ theo kết quả. | Open | 2026-11-27 |
| TD-005 | Khôi phục E2EE ([07](./specs/07-e2ee-cryptography.md)) | Đã mô tả lớp wrap phục hồi nhưng chưa có hợp đồng lấy bundle và đổi thông tin đăng nhập khi mất mật khẩu. | High | Đặc tả và kiểm thử luồng xác thực phục hồi, cập nhật nguyên tử thông tin đăng nhập và thu hồi phiên trước khi nghiệm thu Phase 3. | Open | 2026-11-27 |
| TD-006 | macOS / `scripts/build-macos-core.sh` | XCFramework phát triển chỉ có kiến trúc máy build. Swift bindings và thư viện Rust cần được tạo lại bằng script khi mã Rust thay đổi. Cách này giữ bước sinh mã tách khỏi Xcode trong giai đoạn khởi tạo. | Low | Tích hợp bước build Rust vào quy trình build app. Tạo và kiểm tra XCFramework cho cả Apple Silicon và Intel khi chuẩn bị phân phối. | Open | 2026-11-27 |
| TD-007 | Windows / [`Notes.csproj`](../clients/windows/Notes/Notes.csproj) | Template WinUI dùng phiên bản NuGet `*` cho Windows App SDK và hai gói BuildTools. Lần restore trên máy mới có thể chọn phiên bản khác với bản đã kiểm tra. | Low | Ghim phiên bản các gói NuGet khi thiết lập build Windows trong CI và kiểm tra restore trên máy sạch. | Open | 2026-11-28 |
| TD-008 | Windows / [`PrototypeEditor.cs`](../clients/windows/Notes/PrototypeEditor.cs) | Generator C# 0.32 không đọc được metadata UniFFI 0.33. WinUI dùng P/Invoke thủ công, JSON và handle tự quản lý để chạy M0. | Medium | Thử lại generator cùng phiên bản UniFFI. Nếu giữ C ABI, sinh khai báo và kiểm thử vòng đời DLL/handle trên các kiến trúc phân phối. | Open | 2026-11-28 |
| TD-009 | [Rust editor](../core/notes-app/src/editor.rs) và adapter native | M0 dựng bản staging toàn tài liệu để bảo đảm batch nguyên tử. Checklist và bảng dùng control riêng cho từng mục/ô. Lịch sử theo block có thể báo xung đột khi external patch chạm block đã sửa. | Medium | Đo với tài liệu dài. Đánh giá giao dịch CRDT gia tăng, undo ở độ hạt thao tác và cách render bảng trong M1/M2. | Open | 2026-11-28 |
| TD-010 | SQLite / [`m0_commit.rs`](../core/notes-app/examples/m0_commit.rs) | Benchmark lưu snapshot fixture và journal trên một kết nối. Client M0 chỉ hiển thị revision của editor trong bộ nhớ để tách phép đo SQLite khỏi adapter. | Medium | Tích hợp storage vào luồng editor M1. Kiểm tra ACK, flush khi app đổi vòng đời và retry từ API thật trên hai OS. | Open | 2026-11-28 |
| TD-011 | WinUI / [`MainPage.xaml.cs`](../clients/windows/Notes/MainPage.xaml.cs) | UniKey trên layout US gửi sửa đổi từng phím, nên WinUI không nhận được ranh giới composition. Trong lượt thử, F7 giữa `tieeng` và `s` làm chữ thành `tiêngs`. Vietnamese Telex tích hợp đạt ca tương ứng. | Medium | Xác định chính sách patch cùng control với các bộ gõ không phát sự kiện composition. Kiểm tra lại UniKey cùng phương án giữ trạng thái gõ. | Open | 2026-11-28 |

---

## Resolved Technical Debt

| ID | Khu vực / File | Mô tả | Cách đã xử lý | Ngày hoàn thành |
| :--- | :--- | :--- | :--- | :--- |
| — | — | — | — | — |
