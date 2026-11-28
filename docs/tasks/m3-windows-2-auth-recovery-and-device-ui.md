# M3-W.2 · Giao diện đăng nhập E2EE, phục hồi và quản lý thiết bị

**Phụ thuộc** — [M3-W.1](./m3-windows-1-sync-ffi-and-secure-storage.md), backend self-host hỗ trợ auth/keys/devices và đặc tả phục hồi tài khoản (`TD-005`).

## Phạm vi

Xây dựng các màn hình và hộp thoại WinUI 3 cho vòng đời tài khoản E2EE:
- Cấu hình địa chỉ máy chủ self-host (bắt buộc HTTPS, kiểm tra kết nối/health).
- Đăng ký tài khoản mới (nhập email, passphrase, sinh `kdf_salt` + `AMK`, hiển thị và xác nhận người dùng đã lưu **Recovery Key** một lần duy nhất).
- Đăng nhập tài khoản có sẵn (`POST /v1/auth/salt` → derive `AK`/`KEK` → `POST /v1/auth/login` → unwrap `AMK`), hỗ trợ đưa các ghi chú/thư mục/ảnh đã tạo offline từ M2 vào outbox khi bật đồng bộ lần đầu.
- Đổi mật khẩu (`PUT /v1/auth/credentials`), phục hồi tài khoản bằng Recovery Key (`TD-005`), và màn hình Quản lý thiết bị (hiển thị danh sách thiết bị, thu hồi thiết bị khác qua `DELETE /v1/devices/{id}`, xử lý khi chính thiết bị Windows hiện tại bị thu hồi từ máy khác).

## Tiêu chí nghiệm thu

- [ ] Người dùng đăng ký tài khoản mới trên WinUI, nhận Recovery Key (24 từ BIP-39 hoặc Base32 kèm checksum) và đăng nhập lại thành công trên cùng máy hoặc máy khác.
- [ ] Khi bật đồng bộ lần đầu trên kho dữ liệu offline đã có ghi chú, thư mục và ảnh từ M2, các mục hợp lệ (không bị khóa) được đưa vào outbox và đẩy lên server đầy đủ.
- [ ] Đổi mật khẩu thành công trên Windows: cập nhật `wrapped_amk` và `auth_verifier` mà không cần mã hóa lại toàn bộ ghi chú; phiên ở thiết bị khác bị thu hồi theo spec.
- [ ] Luồng quên mật khẩu + Recovery Key khôi phục được `AMK`, đặt mật khẩu mới và tiếp tục đồng bộ bình thường.
- [ ] Thu hồi thiết bị từ giao diện Windows vô hiệu hóa quyền truy cập API của thiết bị bị thu hồi; nếu chính máy Windows đang dùng bị thu hồi, app nhận lỗi xác thực, xóa token/phiên cục bộ an toàn nhưng vẫn giữ dữ liệu ghi chú offline và thông báo người dùng đăng nhập lại.

[Kế hoạch M3 Windows](../plans/20261128144800-m3-windows-sync.md) · [Đặc tả E2EE](../specs/07-e2ee-cryptography.md#6-xác-thực-zero-knowledge-authentication-flow) · [Backend API](../specs/06-backend-spring-boot.md#45-auth--e2ee-key-management-endpoints)
