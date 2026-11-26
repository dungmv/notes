# Agent Rules & Project Conventions

## 1. Implementation Plans (`docs/plans/`)

- Mọi kế hoạch triển khai (implementation plan) **bắt buộc** phải lưu thành file Markdown trong thư mục `docs/plans/`.
- Quy tắc đặt tên file: `yyyymmddhhmmss-<plan-name>.md` (sử dụng timestamp thời điểm tạo theo định dạng 14 chữ số + tên ngắn gọn viết thường, nối bằng dấu gạch ngang).
  - Ví dụ: `docs/plans/20261127030500-init-rust-workspace.md`.

## 2. Technical Debt Tracking (`docs/technical-debt.md`)

- **Không được bỏ qua nợ kỹ thuật (No Silent Technical Debt)**: Mọi giải pháp tạm thời (workaround), đánh đổi kiến trúc (trade-off), phần chưa tối ưu, test bị hoãn, hoặc việc cần refactor sau đều **phải được ghi nhận** vào `docs/technical-debt.md`.
- Mỗi mục nợ kỹ thuật cần ghi rõ:
  - **ID** (ví dụ: `TD-001`)
  - **Khu vực / File liên quan**
  - **Mô tả vấn đề & Lý do tồn tại**
  - **Mức độ ưu tiên** (`High` / `Medium` / `Low`)
  - **Hướng xử lý đề xuất & Trạng thái** (`Open` / `In Progress` / `Resolved`)
- Khi xử lý xong một mục nợ kỹ thuật, cập nhật trạng thái sang `Resolved` (kèm ngày hoàn thành), không xóa lịch sử để tiện đối chiếu.
