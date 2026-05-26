# Video Downloader

Full-stack video downloader workspace with a Next.js frontend and a Rust Axum
backend. The app accepts YouTube channel/playlist URLs and TikTok profile URLs,
lists videos, lets the user select one or many items, and streams a ZIP archive
back to the browser.

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
    channel/            YouTube/TikTok channel and profile crawlers
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

Fetches a single YouTube or TikTok video page and returns normalized metadata:

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
- streamed ZIP body

The backend starts download tasks concurrently and streams ZIP bytes through the
HTTP response instead of writing temporary files or buffering the full archive.

## Frontend Workflow

1. Paste a YouTube channel/playlist or TikTok profile URL.
2. Click `Fetch`.
3. Select individual videos or use `Chọn tất cả`.
4. Click `Tải xuống x video`.
5. Save the returned ZIP file.

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
  fixtures for YouTube and TikTok parsing/pagination behavior.
- The bulk ZIP writer currently targets normal ZIP archives, not Zip64.
- This project intentionally does not use `yt-dlp` or provider wrapper modules.
