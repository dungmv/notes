# M4-W.8 · Nghiệm thu Desktop 1.0 phần Windows

**Phụ thuộc** — [M4-W.1](./m4-windows-1-locked-notes-core-and-sqlcipher.md) đến [M4-W.7](./m4-windows-7-backend-backup-restore-and-compaction-decision.md) và bản cài MSIX từ M4-W.6.

## Phạm vi

- Chạy bộ ca nghiệm thu chính thức **liên tính năng** trên bản cài MSIX (không phải bản dev): khóa ghi chú × tìm kiếm/Gallery × khởi động lại; khóa × sync (bị chặn đúng cả hai hướng, `TD-003`); import/export với ghi chú đã đồng bộ; nâng cấp từ bản M3 trên dữ liệu thật; đồng bộ tiếp sau khi server được khôi phục từ backup (M4-W.7).
- Hành vi tham chiếu [Phase 4+](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu): danh sách hiện tiêu đề và trạng thái khóa; tìm theo nội dung, tag và bộ lọc attachment không tiết lộ dữ liệu đã khóa; hủy xác thực đưa editor về màn hình khóa và bỏ phiên mở khóa.
- Tổng hợp số đo trên bản cài: cold start tới khi gõ được, độ trễ gõ trên tài liệu có bảng/tag, thời gian khóa/mở khóa, thời gian khôi phục server; đối chiếu baseline [M2](../m2-windows-acceptance.md).
- Lập biên bản nghiệm thu `docs/m4-windows-acceptance.md` theo mẫu biên bản M2 (môi trường, dữ liệu, ca đạt/chưa đạt, số đo, quyết định).
- Cập nhật [sổ nợ kỹ thuật](../technical-debt.md): `TD-002` (quyết định compaction), `TD-003` (ghi nhận phần cục bộ xong, phần sync giữ Open), `TD-008`, `TD-009`, `TD-012`, `TD-014` và các nợ mới phát hiện trong mốc.

## Tiêu chí nghiệm thu

- [ ] Toàn bộ ca liên tính năng đạt trên bản cài MSIX; ca chưa đạt có ghi rõ điều kiện tái hiện và mục nợ tương ứng.
- [ ] Không có plaintext nội dung ghi chú khóa trên đĩa (DB, WAL, blob, temp, log) được xác minh bằng kiểm tra thực tế trên bản cài.
- [ ] Bảng số đo hiệu năng hoàn chỉnh với cấu hình máy, phiên bản Windows, bản build và so sánh baseline M2.
- [ ] Biên bản `docs/m4-windows-acceptance.md` được lập, kèm link môi trường và log; roadmap M4 ghi nhận kết quả phần Windows.
- [ ] `docs/technical-debt.md` cập nhật trạng thái mọi mục liên quan; các điều kiện còn thiếu để đóng M4 đầy đủ (phần macOS) được liệt kê rõ.

[Kế hoạch M4](../plans/20261128183931-m4-desktop-1.0-windows.md) · [Roadmap M4](../roadmap.md#m4--desktop-10) · [Hành vi tham chiếu](../specs/03-features-and-editor.md#0-hành-vi-tham-chiếu-và-nghiệm-thu)
