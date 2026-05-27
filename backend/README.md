# Backend

Rust Axum REST API that acts as the core extraction engine and bulk download coordinator for the Video Downloader workspace.

## Stack & Tech Choice

- **Framework**: `axum` with `tokio` asynchronous runtime.
- **HTTP Client**: `reqwest` for raw content fetches.
- **Data Serialization**: `serde` & `serde_json` for processing payloads and internal structures.
- **Archive Streamer**: In-memory `zip` writer integrated with Axum's chunked response body.

## API Endpoints

### 1. Health Verification
- **Route**: `GET /api/health`
- **Output**: Plain text `OK: backend is healthy` (with CORS headers matching the frontend origin).

### 2. Single Video Extraction
- **Route**: `GET /api/extract?url={link}&cookie={optional}`
- **Behavior**:
  - **YouTube**: Sends a `POST` request to the Android InnerTube player endpoint (`/youtubei/v1/player`) to retrieve un-deciphered direct MP4 URLs.
  - **TikTok**: Scrapes watch page HTML to find and parse `SIGI_STATE` or `__NEXT_DATA__` blocks containing no-watermark streams.
  - **Facebook**: Scrapes the page source to resolve standard/high definition stream URLs.
  - Optional `cookie` query parameter bypasses verification barriers/auth-walls.

### 3. Collection Crawler (Channels, Profiles, Playlists)
- **Route**: `GET /api/channel?url={link}&cookie={optional}`
- **Behavior**: Traverses paginated items using cursor offsets or continuation tokens:
  - **YouTube**: Parses continuation tokens and crawls `youtubei/v1/browse` pages.
  - **TikTok**: Resolves profile hydration models using cursor steps.
  - **Facebook**: Scrapes video IDs on Page/Profile feeds using page cursor parameters.

### 4. Bulk Download ZIP Streamer
- **Route**: `POST /api/download/bulk`
- **Payload**:
  ```json
  {
    "source_url": "https://www.youtube.com/@channel/videos",
    "cookie": "optional-cookie-string",
    "ids": ["video-id-1", "video-id-2"]
  }
  ```
- **Behavior**: Starts concurrent download tasks to fetch media segments. It pipes compressed bytes directly to the caller as an `application/zip` stream. No intermediate temporary files are written to disk.

---

## Commands

### Run Dev Server
```bash
cargo run
```
The server binds and listens on `http://localhost:8080`.

### Run Test Suite
```bash
cargo test --offline
```

### Check Formatting & Types
```bash
cargo fmt --check
cargo check --offline
```
