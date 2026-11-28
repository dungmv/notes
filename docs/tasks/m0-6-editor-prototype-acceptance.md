# M0.6 · Nghiệm thu M0 và chốt lựa chọn

**Phụ thuộc** — [M0.3 · macOS](./m0-3-macos-editor-prototype.md), [M0.4 · Windows](./m0-4-windows-editor-prototype.md) và [M0.5 · Đo commit SQLite](./m0-5-sqlite-commit-benchmark.md).

Tổng hợp kết quả theo từng ca và từng nền tảng. Dùng bằng chứng chạy native và số đo để chốt adapter, binding, cách gom batch và các thay đổi cần đưa sang M1/M2.

## Tiêu chí nghiệm thu

- [ ] Có kết quả, bằng chứng và cách tái hiện từng ca. Các ca bắt buộc đạt trên cả macOS và Windows.
- [ ] Có tài liệu ghi quyết định kỹ thuật cùng số đo, giới hạn và ảnh hưởng đến M1/M2.
- [ ] Cập nhật sổ nợ kỹ thuật. Chuyển `TD-001` sang `Resolved` khi đủ kết quả Windows và số đo batch.

[Biên bản nghiệm thu đang mở](../m0-editor-prototype-results.md) lưu số đo, bằng chứng và các quyết định tạm thời. `TD-001` ở trạng thái `In Progress` trong khi chờ các ca native còn thiếu.

[Kế hoạch M0](../plans/20261127174136-m0-editor-ffi-prototype.md#đo-lường-và-điều-kiện-đóng-m0) · [Sổ nợ kỹ thuật](../technical-debt.md)
