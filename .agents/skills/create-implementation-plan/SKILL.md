---
name: create-implementation-plan
description: Tạo hoặc cập nhật kế hoạch triển khai trong repo Notes, kèm mô tả đầu vào, tiêu chí nghiệm thu đầu ra và báo cáo tiến độ theo task.
---

# Lập kế hoạch triển khai

Đọc `AGENTS.md` và các spec, kế hoạch, task hoặc mã nguồn liên quan để xác định điểm xuất phát. Nếu yêu cầu chỉ lập hoặc cập nhật kế hoạch, lưu tài liệu và báo cáo trạng thái hiện tại. Triển khai khi yêu cầu của người dùng bao gồm việc thực hiện.

## Xác nhận đầu vào và đầu ra

Sau khi đối chiếu tài liệu liên quan, nếu còn thiếu thông tin hoặc có nhiều cách hiểu về đầu vào, đầu ra hay tiêu chí nghiệm thu, hỏi người dùng để xác nhận. Nêu cụ thể thông tin cần làm rõ và phần kế hoạch phụ thuộc vào câu trả lời. Chỉ điền thông tin có căn cứ hoặc đã được người dùng xác nhận. Nếu cần lưu bản nháp trong lúc chờ, đánh dấu mục liên quan là **Chờ xác nhận**, ghi câu hỏi còn mở và chỉ chốt kế hoạch khi đã có câu trả lời.

## Tạo kế hoạch

Lưu kế hoạch vào `docs/plans/yyyymmddhhmmss-<ten-ke-hoach>.md`. Dùng thời điểm tạo thực tế theo múi giờ dự án và tên ngắn viết thường, nối bằng dấu gạch ngang. Khi cập nhật kế hoạch hiện có, giữ tên file.

- **Mục tiêu và phạm vi** — Nêu kết quả cần đạt và giới hạn đã được thống nhất.
- **Đầu vào** — Mô tả yêu cầu, hiện trạng, dữ liệu hoặc môi trường cần có, phụ thuộc và ràng buộc liên quan. Liên kết tài liệu, file hoặc task nguồn khi có. Ghi rõ các giả định đã được xác nhận và các mục đang chờ xác nhận.
- **Đầu ra và tiêu chí nghiệm thu** — Liệt kê sản phẩm bàn giao. Với mỗi đầu ra, dùng checklist nêu điều kiện đạt quan sát được và cách xác minh, chẳng hạn bằng lệnh kiểm tra, ca thao tác hoặc tài liệu đối chiếu. Ghi ngưỡng đo khi yêu cầu hoặc baseline đã xác định ngưỡng.
- **Các task triển khai** — Ghi mã hoặc tên, kết quả cần giao và phụ thuộc theo thứ tự thực hiện. Mỗi task có file riêng trong `docs/tasks/` ghi phạm vi, phụ thuộc và tiêu chí nghiệm thu. Liên kết hai chiều giữa kế hoạch và từng task. Dùng lại file nếu task đã được ghi nhận.
- **Trạng thái và tiến độ** — Ghi ngày cập nhật, trạng thái chung, số task hoàn thành trên tổng số task và bảng bên dưới. Phản ánh hiện trạng có bằng chứng khi lập kế hoạch.

| Task | Trạng thái | Kết quả kiểm chứng hoặc vướng mắc | Bước tiếp theo |
| --- | --- | --- | --- |

## Cập nhật tiến độ

- Dùng các trạng thái **Chưa bắt đầu**, **Đang thực hiện**, **Bị chặn**, **Hoàn thành**. Khi một mốc thay đổi hoặc có vướng mắc ảnh hưởng đến kế hoạch, cập nhật bảng tiến độ và checklist liên quan trong file kế hoạch và file task. Ghi rõ điều kiện để gỡ vướng.
- Chỉ tính task hoàn thành khi tiêu chí nghiệm thu của task đã đạt. Ghi bằng chứng kiểm chứng trong file task. Chỉ đóng kế hoạch khi mọi task và tiêu chí nghiệm thu đầu ra đều đạt.
- Ghi nợ kỹ thuật phát sinh và cập nhật nợ đã xử lý trong `docs/technical-debt.md` theo `AGENTS.md`.
- Khi báo cáo cho người dùng, liên kết file kế hoạch và tóm tắt số task hoàn thành, kết quả đã kiểm chứng, vướng mắc hiện tại và bước tiếp theo nếu còn việc.
