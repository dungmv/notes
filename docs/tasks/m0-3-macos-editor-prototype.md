# M0.3 · Dựng prototype editor macOS

**Phụ thuộc** — [M0.2 · Hợp đồng editor Rust và FFI](./m0-2-rust-editor-ffi.md).

Bọc `NSTextView` trong SwiftUI, kết nối với Rust qua UniFFI và áp patch gia tăng. Xác minh TextKit 2 đang hoạt động trong các luồng thử.

## Tiêu chí nghiệm thu

- [ ] Chạy bộ ca chung trên macOS, gồm IME tiếng Việt, emoji, vùng chọn, undo/redo, checklist và bảng cơ bản.
- [ ] Lưu kết quả từng ca, cấu hình máy, phiên bản macOS và số đo độ trễ gõ, áp patch.

[Kế hoạch M0](../plans/20261127174136-m0-editor-ffi-prototype.md#bộ-ca-nghiệm-thu)
