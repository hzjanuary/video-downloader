# Epic 05: Facebook Extraction

## Mục tiêu
Trích xuất metadata và cào danh sách video từ Facebook (Video/Reels) và Facebook Page/Profile.

## Nội dung (US-011, US-012)
1. Parse link đơn (US-011): Viết parser Rust cho Facebook Video và Facebook Reels. Tải HTML. Phân tích DOM hoặc data Graph API ẩn để lấy link MP4.
2. Cào Page/Profile (US-012): Viết logic phân trang cào mảng video ID từ link Facebook Page/Profile.

## Yêu cầu Kỹ thuật
* Kế thừa: Tái sử dụng model `VideoInfo` (E02) và endpoint `/api/extract`, `/api/channel` (E03).
* Chống chặn: Facebook chặn gắt HTTP client cơ bản. Cấu hình `reqwest` fake User-Agent chuẩn. Thêm tính năng nhận tham số Cookie (tùy chọn từ UI) để vượt auth-wall nếu bị chặn.