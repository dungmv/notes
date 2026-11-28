# M3-W.1 · FFI đồng bộ và kho khóa an toàn Windows

**Phụ thuộc** — [M2.1 · Build Windows và FFI offline](./m2-1-windows-build-and-offline-ffi.md) và hợp đồng FFI đồng bộ (`notes-sync`, `notes-crypto`, event listener) trong `core/notes-ffi`.

## Phạm vi

Mở rộng lớp cầu nối C# (`OfflineSession.cs` / `SyncSession.cs`) trên WinUI 3 để hỗ trợ các lệnh xác thực, quản lý khóa, kích hoạt đồng bộ bất đồng bộ và nhận luồng sự kiện từ Rust core về `DispatcherQueue`. Tách các tác vụ lưu và đồng bộ khỏi UI thread để giữ editor phản hồi mượt (liên quan `TD-014`). Triển khai lớp lưu trữ khóa an toàn trên Windows bằng DPAPI (`DataProtectionProvider` / `ProtectedData`) hoặc `PasswordVault` cho `Account Master Key (AMK)` và refresh token, đảm bảo không lưu secret dạng rõ trên đĩa và xóa buffer secret sau khi truyền qua FFI.

## Tiêu chí nghiệm thu

- [ ] Lớp C# gọi được các lệnh auth, key bundle, `trigger_sync`, lấy trạng thái đồng bộ và quản lý thiết bị qua `notes_ffi.dll` với quản lý `SafeHandle` và giải phóng buffer đúng quy định, không rò rỉ bộ nhớ qua nhiều vòng mở/đóng phiên.
- [ ] Event callback từ background thread của Rust (`SyncStateChanged`, `NoteContentUpdated`, `FoldersChanged`, `NoteListInvalidated`, `ErrorOccurred`) được chuyển an toàn về UI thread qua `DispatcherQueue.TryEnqueue` và hủy đăng ký sạch sẽ khi đóng cửa sổ/phiên.
- [ ] Tác vụ ghi và đồng bộ chạy trên worker theo thứ tự, không block UI thread khi đang gõ trên `RichEditBox`.
- [ ] `AMK` và refresh token được mã hóa/lưu qua Windows DPAPI / `PasswordVault` theo người dùng Windows hiện tại; mở lại app tự mở khóa phiên sync bằng khóa đã lưu mà không cần nhập lại mật khẩu (trừ khi phiên bị thu hồi hoặc hết hạn).
- [ ] Secret truyền từ C# sang Rust qua FFI được xóa khỏi bộ nhớ ngay sau khi nạp xong; không ghi `AMK`, `KEK`, `AK` hay passphrase ra log ứng dụng.

[Kế hoạch M3 Windows](../plans/20261128144800-m3-windows-sync.md) · [Kiến trúc FFI](../specs/01-architecture-and-ffi.md) · [Đặc tả E2EE](../specs/07-e2ee-cryptography.md#72-tích-hợp-os-secure-storage--biometrics)
