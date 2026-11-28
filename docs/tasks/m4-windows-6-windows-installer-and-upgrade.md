# M4-W.6 · Bản cài Windows MSIX, cài mới và nâng cấp

**Phụ thuộc** — [M4-W.2](./m4-windows-2-locked-notes-ui-windows-hello.md), [M4-W.4](./m4-windows-4-organization-ui-gallery-quick-note.md), [M4-W.5](./m4-windows-5-import-export.md) (đủ tính năng trước khi đóng gói).

## Phạm vi

- Đóng gói MSIX theo [05 §2](../specs/05-platform-clients-and-roadmap.md#2-windows-client-winui-3--windows-app-sdk): manifest (identity, version, capabilities), chứng chỉ ký, tích hợp bước build `notes_ffi.dll` đúng kiến trúc vào package; kiểm tra bản app/DLL đúng kiến trúc trước khi công bố hỗ trợ (`TD-007`, `TD-008`).
- Cài mới trên môi trường sạch (Windows Sandbox hoặc VM): dữ liệu vào `%LOCALAPPDATA%\Notes`, quyền tối thiểu, chạy được luồng offline cơ bản ngay sau cài.
- Nâng cấp từ bản trước: cài đè phiên bản M3 giữ nguyên dữ liệu, migration schema chạy đúng, kho SQLCipher và dữ liệu đồng bộ (token, outbox, khóa) còn nguyên sau nâng cấp; quy trình rollback ghi rõ.
- Trải nghiệm trên bản cài: phím tắt theo quy ước Windows (`Ctrl+N`, `Ctrl+F`, `Ctrl+Shift+F`, `Ctrl+Shift+L`, `Ctrl+K`, `Win+Alt+N`), focus bàn phím trong ba cột, Narrator ở các luồng chính (tạo note, checklist, khóa note, tìm kiếm) theo [chiến lược kiểm thử](../specs/05-platform-clients-and-roadmap.md#4-chiến-lược-kiểm-thử-testing-strategy).
- Ghi nhận các bước còn phụ thuộc runner/CI sạch (`TD-007` đang In Progress) và phần nào nằm ngoài khả năng môi trường hiện tại.

## Tiêu chí nghiệm thu

- [ ] MSIX cài thành công trên máy sạch từ file cài (kể cả double-click certificate), app khởi chạy và đọc/ghi dữ liệu vào đúng thư mục.
- [ ] Nâng cấp từ bản M3 sang bản M4 trên dữ liệu thật: ghi chú, ảnh, thư mục, cấu hình đồng bộ và ghi chú khóa (nếu có) còn nguyên; mở lại và đồng bộ tiếp đúng.
- [ ] Gỡ cài đặt không xóa dữ liệu người dùng ngoài ý muốn; cài lại tìm lại được dữ liệu cũ.
- [ ] Toàn bộ phím tắt chính hoạt động trên bản cài; điều hướng bàn phím ba cột không kẹt focus; Narrator đọc được tên và trạng thái ở các luồng chính.
- [ ] Package không kèm DLL sai kiến trúc; việc build/đóng gói có thể lặp lại bằng một script hoặc lệnh duy nhất từ root repo.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Client Windows](../specs/05-platform-clients-and-roadmap.md#2-windows-client-winui-3--windows-app-sdk) · [Sổ nợ kỹ thuật](../technical-debt.md)

## Trạng thái thực thi (2026-11-28)

- Hoàn thành phần chuẩn bị: build self-contained standalone qua `build-windows-core.ps1 -StandaloneOutput target/m4-installer`; DLL đúng kiến trúc kiểm tra trong script (TD-007 tiếp tục In Progress).
- Kiểm chứng nâng cấp dữ liệu: migration plaintext → SQLCipher và schema v1→v6 có test Rust (`plaintext_store_migrates_and_wrong_key_fails`, `v1_migrates_from_m0_document_and_rolls_back_on_sql_error` mở rộng v6).
- Còn lại: MSIX ký chứng chỉ + cài mới trên máy sạch + Narrator/phím tắt trên bản cài + chạy lại toàn bộ ma trận M2 trên bản M4 (để W.8).
