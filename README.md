# Video Downloader

Full-stack video downloader workspace with a Next.js frontend and a Rust Axum
backend. The app accepts YouTube channel/playlist URLs, TikTok profile URLs, and
Facebook video/Page/Profile URLs, lists videos, lets the user select one or many
items, and streams a ZIP archive back to the browser.

This repository also includes Harness documentation and story tracking under
`docs/` and `scripts/harness`.

## Current Features

- Next.js App Router frontend in `frontend`.
- Rust Axum backend in `backend`.
- Health check: `GET /api/health`.
- Single video extraction: `GET /api/extract?url={link}`.
- Channel/profile listing: `GET /api/channel?url={link}`.
- Bulk download: `POST /api/download/bulk`.
- YouTube channel/user/handle/playlist pagination through continuation tokens.
- TikTok profile pagination through cursor/max_cursor.
- Facebook single video/Reels MP4 extraction and Page/Profile video listing
  from exposed HTML/JSON data.
- Optional provider cookie forwarding for Facebook auth-wall cases.
- Streaming ZIP response with concurrent Tokio download tasks.

## Tech Stack

| Surface | Stack |
| --- | --- |
| Frontend | Next.js, React, TypeScript |
| Backend | Rust, Axum, Tokio |
| HTTP client | `reqwest` |
| Parsing | `regex`, `serde_json` |
| Validation | Cargo tests, Next build/typecheck, Harness matrix |

## Repository Layout

```text
frontend/
  app/                  Next.js bulk download UI

backend/
  src/
    bulk.rs             ZIP streaming bulk download worker
    channel/            YouTube/TikTok/Facebook collection crawlers
    extract/            Single-video extraction parsers
    main.rs             Axum routes and app wiring
    model.rs            Shared response models

docs/
  product/              Product contracts
  stories/              Story packets and validation evidence
  HARNESS.md            Harness operating guide

scripts/
  harness               Stable Harness CLI entrypoint
```

## Requirements

- Node.js and npm.
- Rust toolchain with Cargo.
- The Harness CLI already installed at `scripts/bin/harness-cli`.

## Run Locally

Install frontend dependencies:

```bash
cd frontend
npm install
```

Start the backend:

```bash
cd backend
cargo run
```

Start the frontend in another terminal:

```bash
cd frontend
npm run dev
```

Open:

```text
http://localhost:3000
```

The backend listens on:

```text
http://localhost:8080
```

The frontend reads `NEXT_PUBLIC_API_BASE_URL` and defaults to
`http://localhost:8080`.

## API

### `GET /api/health`

Returns plain text:

```text
OK: backend is healthy
```

### `GET /api/extract?url={link}`

Fetches a single YouTube, TikTok, or Facebook video page and returns normalized
metadata:

```json
{
  "platform": "youtube",
  "source_url": "https://www.youtube.com/watch?v=abc123",
  "id": "abc123",
  "title": "Example video",
  "author": "Example channel",
  "duration_seconds": 61,
  "thumbnail_url": "https://example.com/thumb.jpg",
  "streams": []
}
```

### `GET /api/channel?url={link}`

Returns a list of short video metadata:

```json
[
  {
    "id": "video-id",
    "title": "Video title",
    "thumbnail_url": "https://example.com/thumb.jpg"
  }
]
```

Supported inputs:

- YouTube `/channel/...`
- YouTube `/c/...`
- YouTube `/user/...`
- YouTube `/@handle/...`
- YouTube `/playlist?list=...`
- TikTok `/@username`
- Facebook Page/Profile video URLs on `facebook.com` / `*.facebook.com`

### `POST /api/download/bulk`

Request:

```json
{
  "source_url": "https://www.youtube.com/@channel/videos",
  "cookie": "optional provider cookie",
  "format": "mp4",
  "quality": "720p",
  "ids": ["video-id-1", "video-id-2"]
}
```

Response:

- `200 application/zip`
- `Content-Disposition: attachment; filename="videos.zip"`
- streamed ZIP body

The backend starts download tasks concurrently and streams ZIP bytes through the
HTTP response instead of writing temporary files or buffering the full archive.
`format` defaults to `mp4`, and `quality` defaults to `best`. `mp3` uses the
local GStreamer `lamemp3enc` pipeline and requires an AAC decoder plugin such as
`avdec_aac`, `faad`, or `fdkaacdec` for common YouTube, TikTok, and Facebook
MP4 sources.

## Frontend Workflow

1. Paste a YouTube channel/playlist, TikTok profile, or Facebook URL.
2. Click `Fetch`.
3. Select individual videos or use `Chọn tất cả`.
4. Choose format and quality.
5. Click `Tải xuống x video`.
6. Save the returned ZIP file.

## Validation

Backend:

```bash
cd backend
cargo fmt --check
cargo check --offline
cargo test --offline
```

Frontend:

```bash
cd frontend
npm run typecheck
npm run build
```

Harness matrix:

```bash
scripts/harness query matrix
```

## Notes

- Provider HTML and pagination APIs are volatile. Tests use deterministic
  fixtures for YouTube, TikTok, and Facebook parsing/pagination behavior.
- The bulk ZIP writer currently targets normal ZIP archives, not Zip64.
- This project intentionally does not use `yt-dlp` or provider wrapper modules.
