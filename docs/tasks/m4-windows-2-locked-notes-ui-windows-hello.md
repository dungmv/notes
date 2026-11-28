# M4-W.2 · Locked Notes UI trên WinUI và Windows Hello

**Phụ thuộc** — [M4-W.1](./m4-windows-1-locked-notes-core-and-sqlcipher.md) (FFI khóa/mở khóa, SQLCipher sẵn sàng).

## Phạm vi

- UI khóa/mở khóa: nút **Lock** trên toolbar editor, dialog đặt/đổi mật khẩu khóa, màn hình khóa thay editor khi ghi chú khóa; hủy xác thực hoặc khóa lại đưa editor về màn hình khóa và bỏ trạng thái phiên mở khóa theo [hành vi tham chiếu Phase 4+](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu).
- Windows Hello theo khả năng thiết bị: mở nhanh qua `UserConsentVerifier`; tùy chọn bật/tắt trong cài đặt; lưu NLK và Local DB Key bằng DPAPI / `PasswordVault` / `KeyCredentialManager` theo [07 §7.2](../specs/07-e2ee-cryptography.md#72-tích-hợp-os-secure-storage--biometrics).
- Danh sách và tìm kiếm: ghi chú khóa hiện tiêu đề, icon khóa và mốc thời gian; filter chip `Locked`; tìm theo nội dung, tag và bộ lọc attachment **không tiết lộ** dữ liệu đã khóa; thumbnail và Gallery bỏ qua nội dung khóa.
- Auto-lock phía client: nút Lock Now, đăng ký sự kiện khóa màn hình/sleep của Windows, bộ đếm thời gian không tương tác (mặc định 5 phút, cấu hình được); chuyển sang khóa mà không phá composition IME tiếng Việt đang soạn ở ghi chú thường.
- Ràng buộc giao diện: thư viện đã bật sync → vô hiệu hóa khóa ghi chú kèm giải thích `TD-003`; thư viện có ghi chú khóa → vô hiệu hóa bật sync.

## Tiêu chí nghiệm thu

- [ ] Toàn bộ luồng khóa → đóng app → mở lại → mở khóa bằng mật khẩu hoặc Windows Hello hoạt động trên bản build WinUI; mở khóa thất bại (sai mật khẩu, từ chối Hello) quay về màn hình khóa an toàn.
- [ ] Danh sách, tìm kiếm (có dấu và không dấu), filter chip, Gallery và thumbnail không hiển thị nội dung/snippet/ảnh của ghi chú khóa; tiêu đề và trạng thái khóa vẫn hiển thị đúng.
- [ ] Auto-lock kích hoạt bởi cả ba sự kiện (Lock Now, hết N phút, khóa màn hình/sleep); sau khóa lại, editor ở màn hình khóa và phiên mở cũ không còn hiệu lực.
- [ ] Kho khóa DPAPI/PasswordVault không rò key material vào log, dump hay crash report; bỏ thiết bị bảo mật (máy không có Windows Hello) vẫn dùng được bằng mật khẩu.
- [ ] Các ràng buộc sync hiển thị đúng cả hai chiều và không có đường vòng UI để khóa ghi chú trên thư viện đã sync.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Client Windows](../specs/05-platform-clients-and-roadmap.md#2-windows-client-winui-3--windows-app-sdk) · [E2EE §7.2](../specs/07-e2ee-cryptography.md#72-tích-hợp-os-secure-storage--biometrics)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành: màn hình khóa thay editor, mở bằng mật khẩu, nút Khóa/Khóa lại, ràng buộc sync hai chiều trên UI, icon khóa trong danh sách, `image_data` cho ảnh ghi chú khóa, DPAPI (`local-db-key.dpapi`), Windows Hello qua `UserConsentVerifier` (tùy chọn nhớ passphrase), auto-lock 5 phút + WM_POWERBROADCAST (ngủ máy) + WTS_SESSION_LOCK (khóa phiên), filter `Locked`.
- Kiểm chứng: nghiệm thu M1 tự động 6/6 PASS trên kho SQLCipher qua app WinUI thật; build x64 xanh.
- Còn lại: ca mở khóa bằng Windows Hello cần máy có thiết bị Hello để nghiệm thu cuối (máy hiện tại CheckAvailability trả false → fallback mật khẩu đã hoạt động).
