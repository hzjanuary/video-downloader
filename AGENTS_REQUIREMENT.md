# Bugfix: YouTube Extraction Failure

## Mục tiêu
Sửa lỗi trích xuất video YouTube bị chặn do cơ chế quét HTML thô và giải mã chữ ký (signature decipher) bị lỗi thời. Chuyển sang sử dụng InnerTube API để lấy link stream trực tiếp không mã hóa.

## Nội dung thực hiện
1. **Thay đổi phương thức lấy dữ liệu**:
   - Thay vì tải HTML và dùng Regex tìm `ytInitialPlayerResponse`, chuyển sang gửi một yêu cầu `POST` trực tiếp tới API nội bộ của YouTube: `https://www.youtube.com/youtubei/v1/player`.
   - Định dạng Payload JSON gửi đi:
     ```json
     {
       "videoId": "ID_VIDEO_TRÍCH_XUẤT",
       "context": {
         "client": {
           "clientName": "ANDROID",
           "clientVersion": "19.11.37"
         }
       }
     }
     ```
2. **Dọn dẹp mã nguồn cũ**:
   - Khi sử dụng client context là `ANDROID`, InnerTube API sẽ trả về các đường dẫn trực tiếp (plain URL) nằm trong `streamingData.formats` và `streamingData.adaptiveFormats` mà không cần giải mã cipher.
   - Xóa bỏ hoàn toàn các hàm cũ không còn sử dụng: `decipher_signature`, `swap`, `signed_cipher_url` trong `backend/src/extract/youtube.rs`.
3. **Cập nhật cấu trúc ánh xạ**:
   - Phân tích cú pháp JSON phản hồi từ InnerTube API và ánh xạ chính xác vào các struct `VideoInfo` và `StreamInfo` hiện tại.
   - Đảm bảo giữ nguyên giao thức đầu ra để frontend không bị ảnh hưởng.

## Yêu cầu kỹ thuật
- Sử dụng client `reqwest` để thực hiện cuộc gọi API dạng `POST`.
- Bổ sung xử lý lỗi đầy đủ nếu cấu trúc JSON phản hồi bị thiếu trường hoặc video không tồn tại/bị giới hạn.