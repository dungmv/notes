# M0.5 · Đo chi phí commit SQLite

**Phụ thuộc** — [M0.2 · Hợp đồng editor Rust và FFI](./m0-2-rust-editor-ffi.md).

Dùng SQLite file cục bộ với WAL và `synchronous=FULL`. Đo thời gian từ lúc thao tác đến xác nhận lưu khi ghi từng thao tác và khi gom batch, dùng cùng fixture và chuỗi thao tác.

## Tiêu chí nghiệm thu

- [x] Lưu lệnh chạy, cấu hình, số lượt, kích thước batch và kết quả p50/p95 cho từng phương án.
- [x] Kiểm tra xác nhận đến sau commit, flush, thứ tự, dữ liệu sau khi dừng tiến trình, lỗi ghi và retry.
- [x] Đề xuất cách gom batch dựa trên số đo và độ bền dữ liệu.

[Số đo SQLite và lựa chọn ban đầu](../m0-editor-prototype-results.md#số-đo-sqlite)

[Kế hoạch M0](../plans/20261127174136-m0-editor-ffi-prototype.md#đo-lường-và-điều-kiện-đóng-m0)
