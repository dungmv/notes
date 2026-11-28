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
| TD-007 | Windows / [`Notes.csproj`](../clients/windows/Notes/Notes.csproj) | Phiên bản Windows App SDK và BuildTools đã được ghim. App self-contained chạy được trên Windows Sandbox sạch sau khi liên kết tĩnh CRT cho Rust và SQLite. Còn ca restore và đóng gói qua CI trên runner sạch. | Low | Chạy restore và build trên runner Windows sạch. | In Progress | 2026-11-28 |
| TD-008 | Windows / [`OfflineSession.cs`](../clients/windows/Notes/OfflineSession.cs) | Generator C# 0.32 không đọc được metadata UniFFI 0.33. M0 và M1 WinUI dùng P/Invoke thủ công, JSON qua reflection và SafeHandle. Publish tắt trimming để giữ DTO phục vụ reflection. | Medium | Thử lại generator cùng phiên bản UniFFI hoặc sinh khai báo C ABI và dùng JSON source generation. Kiểm thử vòng đời DLL/handle trên các kiến trúc phân phối. | Open | 2026-11-28 |
| TD-009 | [Rust editor](../core/notes-app/src/editor.rs) và adapter native | M1 vẫn staging toàn tài liệu và lưu layout theo snapshot. Checklist và bảng dùng control riêng cho từng mục/ô. Lịch sử theo block có thể báo xung đột nếu external patch chạm cùng block. Bài đo 130.000 byte có p95 121,67 ms đến commit SQLite. | Medium | Đánh giá giao dịch CRDT gia tăng, chi phí serialize, undo ở độ hạt thao tác và cách render bảng. | In Progress | 2026-11-28 |
| TD-010 | SQLite và [WinUI offline](./tasks/m1-windows-offline.md) | Windows đã kiểm tra commit, dừng tiến trình sau ACK qua C ABI, clipboard hệ thống và chạy WinUI trong guest không có mạng. Còn nghiệm thu native trên macOS. | Medium | Chạy cùng fixture và ma trận vòng đời trên macOS. | In Progress | 2026-11-28 |
| TD-011 | WinUI / [`MainPage.xaml.cs`](../clients/windows/Notes/MainPage.xaml.cs) | UniKey trên layout US gửi sửa đổi từng phím, nên WinUI không nhận được ranh giới composition. Trong M0, F7 giữa `tieeng` và `s` làm chữ thành `tiêngs`. Vietnamese Telex tích hợp đã đạt ca gõ, commit và tìm kiếm trên đường lưu M1. | Medium | Kiểm tra lại UniKey với phương án giữ trạng thái gõ. | Open | 2026-11-28 |
| TD-012 | Blob store / [`offline.rs`](../core/notes-app/src/offline.rs) | Blob mồ côi sau lỗi ghi được giữ theo hash để retry. Metadata ảnh đã gỡ được giữ cho undo/redo. Kho có thể tăng dung lượng theo thời gian. | Medium | Dọn blob theo tập tham chiếu và lịch sử còn hiệu lực, kiểm tra với nhiều phiên ghi. | Open | 2026-11-28 |
| TD-013 | [Editor WinUI](../clients/windows/Notes/MainPage.Offline.cs) | Liên kết theo ID dùng block riêng tại caret. Tự sắp checklist là thứ tự trình bày trong từng nhóm trên Windows, thứ tự gốc vẫn nằm trong tài liệu. | Medium | Đối chiếu mô hình liên kết inline và cách trình bày checklist ở hai client trong bộ fixture dùng chung. | Open | 2026-11-28 |
| TD-014 | [Lưu editor WinUI](../clients/windows/Notes/MainPage.Offline.cs) | Ghi theo từng thao tác đã kết thúc trên UI thread. Hai phiên sửa cùng ghi chú có thể gặp revision cũ. Phiên giữ bản chờ lưu nhưng chưa có luồng hợp nhất xung đột giữa hai cửa sổ. | Medium | Đo độ trễ từ phím đến frame, chuyển ghi bền vững khỏi UI thread theo thứ tự và bổ sung luồng phục hồi bản nháp khi xung đột. | Open | 2026-11-28 |

---

## Resolved Technical Debt

| ID | Khu vực / File | Mô tả | Cách đã xử lý | Ngày hoàn thành |
| :--- | :--- | :--- | :--- | :--- |
| — | — | — | — | — |
