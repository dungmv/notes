# M0.4 · Dựng prototype editor Windows

**Phụ thuộc** — [M0.2 · Hợp đồng editor Rust và FFI](./m0-2-rust-editor-ffi.md) và môi trường chạy WinUI 3.

Nối `RichEditBox` với hợp đồng Rust dùng chung. Thử UniFFI C# trước và đánh giá C ABI nếu binding không đáp ứng. Nếu `RichEditBox` hụt ca bắt buộc, thử WebView2 cục bộ trên các ca đó.

## Tiêu chí nghiệm thu

- [ ] Chạy bộ ca chung trên Windows, ghi rõ phiên bản OS, adapter, binding và kết quả từng ca.
- [ ] Lưu số đo độ trễ gõ, áp patch và bằng chứng chạy cho phương án được chọn. Ghi rõ các ca chưa chạy nếu còn thiếu môi trường.

[Kế hoạch M0](../plans/20261127174136-m0-editor-ffi-prototype.md#bộ-ca-nghiệm-thu)
