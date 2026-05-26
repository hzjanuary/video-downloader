# Bulk Download Contract

## Scope

Bulk download lets a user enter a YouTube channel/playlist URL or TikTok profile
URL, select videos from the fetched list, and download the selected IDs as one
ZIP archive.

## Frontend

- The first screen is the bulk workflow.
- The form calls `GET /api/channel?url={link}`.
- The UI keeps selected IDs in state.
- Users can select one item, multiple items, or `Chọn tất cả`.
- The download action sends the selected ID array to `POST /api/download/bulk`.
- The browser saves the returned ZIP with streaming file writing when supported
  and Blob fallback otherwise.

## API

### `POST /api/download/bulk`

Request:

```json
{
  "source_url": "https://www.youtube.com/@channel/videos",
  "ids": ["video-id-1", "video-id-2"]
}
```

Response:

- `200 application/zip`
- `Content-Disposition: attachment; filename="videos.zip"`
- Streamed ZIP body.

Errors:

- `400` for empty or oversized ID arrays.
- Streaming failures abort the response stream.

## Backend Streaming

- The backend starts one Tokio task per selected ID.
- Download tasks stream chunks through bounded channels.
- The ZIP writer emits local headers, file chunks, data descriptors, central
  directory records, and EOCD directly to the Axum response body.
- No temporary files are written.
- File bytes are not accumulated as one large buffer before the response.

## Validation

- Route tests verify ZIP entries and contents from fixture download IDs.
- HTTP smoke test posts one URL-form ID pointing at `/api/health`, writes the
  streamed ZIP to `/tmp`, lists it with `unzip -l`, and verifies the entry
  content with `unzip -p`.
