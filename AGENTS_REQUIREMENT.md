# Epic 04: Bulk Download

## Mục tiêu
Xây dựng UI chọn nhiều video. Backend tải song song, nén Zip, stream về client.

## Nội dung (US-009, US-010)
1. Frontend Bulk UI (US-009): Giao diện nhập link. Gọi API cào kênh (E03). Render bảng/grid danh sách video. Thêm Checkbox (chọn 1, chọn nhiều, chọn tất cả). Nút "Tải xuống x video". Hiển thị trạng thái tải.
2. Backend Worker (US-010): API `/api/download/bulk`. Nhận mảng video ID. Dùng `tokio` spawn task tải byte song song. Dùng thư viện (VD: `async-zip`) gom byte stream thành 1 file nén `.zip`. Trả stream nén về frontend.

## Yêu cầu Kỹ thuật
* Frontend: Quản lý state mảng ID chính xác. Xử lý tải file lớn không sập trình duyệt (dùng Blob/Stream).
* Backend: Không lưu file tạm lên RAM hay ổ cứng nếu không cần thiết. Stream thẳng byte từ HTTP Client (reqwest) qua Zip Writer rồi đẩy xuống HTTP Response (Axum) để tối ưu bộ nhớ.