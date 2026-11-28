# M3-W.3 · Vòng lặp đồng bộ, lịch kích hoạt Windows và chỉ báo trạng thái

**Phụ thuộc** — [M3-W.1](./m3-windows-1-sync-ffi-and-secure-storage.md) và [M3-W.2](./m3-windows-2-auth-recovery-and-device-ui.md).

## Phạm vi

Tích hợp lịch kích hoạt đồng bộ chuẩn desktop theo đặc tả trên WinUI 3 và hiển thị trạng thái đồng bộ rõ ràng trên thanh công cụ:
- Nút **Đồng bộ ngay** gọi `trigger_sync()` trên thanh công cụ (kèm phím tắt chuẩn nếu có).
- Tách biệt rõ trên UI giữa trạng thái **Đã lưu trên thiết bị** (ACK từ SQLite cục bộ) và trạng thái đồng bộ (**Đang đồng bộ**, **Đã đồng bộ**, **Chờ mạng**, **Lỗi đồng bộ**).
- Kích hoạt đồng bộ tự động khi: mở app, cửa sổ `MainWindow` chuyển sang trạng thái active (`Activated`), mạng có trở lại (`NetworkInformation.NetworkStatusChanged`), và sau một batch chỉnh sửa đã commit cục bộ.
- Chạy polling định kỳ mặc định mỗi 15 giây khi app đang hoạt động và có mạng; tự tạm dừng polling khi cửa sổ không hoạt động hoặc mất mạng; gộp các trigger trùng nhau vào một lượt sync duy nhất tại một thời điểm.
- Ghi log nhẹ (không chứa plaintext/key) phục vụ đo số lượt pull rỗng và độ trễ polling (`TD-004`).

## Tiêu chí nghiệm thu

- [ ] UI phân biệt rõ **Đã lưu trên thiết bị** với **Chờ mạng** / **Đang đồng bộ** / **Đã đồng bộ**; khi tắt mạng và sửa ghi chú, app vẫn báo đã lưu cục bộ và chuyển trạng thái đồng bộ sang **Chờ mạng**.
- [ ] Bấm **Đồng bộ ngay** nhiều lần liên tiếp hoặc kích hoạt đồng bộ trùng thời điểm chỉ chạy một lượt sync tại một thời điểm; nếu có thay đổi commit sau khi lượt hiện tại đã chốt outbox, hệ thống tự lên lịch lượt kế tiếp.
- [ ] Khi bật lại mạng hoặc đưa cửa sổ Windows từ nền lên foreground, app tự động kích hoạt đồng bộ và đẩy hết outbox đang chờ.
- [ ] Polling 15 giây hoạt động khi cửa sổ active và tự dừng khi cửa sổ bị thu nhỏ/mất focus kéo dài hoặc khi mất mạng.
- [ ] Có dữ liệu đo số lượt pull rỗng và thời gian mỗi lượt poll trên Windows để đối chiếu `TD-004`.

[Kế hoạch M3 Windows](../plans/20261128144800-m3-windows-sync.md) · [Lịch đồng bộ](../specs/04-sync-and-security.md#13-lịch-đồng-bộ-trên-client) · [Sổ nợ kỹ thuật](../technical-debt.md)
