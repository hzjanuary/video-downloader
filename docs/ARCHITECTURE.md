# Architecture

This document describes the design, stack choices, and codebase structure of the Video Downloader project.

## Runtime Surfaces & Stack

The application is structured as a decoupled full-stack application:

| Surface | Path | Technology Stack | Purpose |
| --- | --- | --- | --- |
| **Frontend** | `frontend/` | Next.js (App Router), React, TypeScript, TailwindCSS/Vanilla CSS | User interface, video selection grid, file save trigger |
| **Backend** | `backend/` | Rust, Axum, Tokio, Reqwest, Serde | Extractor and playlist crawler API, concurrent media downloader, streaming ZIP archiver |

```text
+-----------------------+                    +------------------------+
|  Next.js Frontend     |                    |  Rust Axum Backend     |
|                       |                    |                        |
|  [ Video Input Form ] |                    |  [ GET /api/extract ]  |
|                       |  HTTP GET/POST     |  [ GET /api/channel ]  |
|  [ Video Grid /     ] | -----------------> |                        |
|  [ Selection State  ] |                    |  [ POST /api/download/bulk ] |
|                       | <----------------- |  - Tokio tasks         |
|  [ ZIP Downloader   ] |   ZIP Stream / JSON|  - zip-writer stream   |
+-----------------------+                    +------------------------+
```

---

## Codebase Layout

### Backend Structure (`backend/src/`)

The backend is built around independent modules coordinated via Axum routing and shared state:

- [main.rs](file:///media/hzjnauary/Workspace/videodownloader/backend/src/main.rs): Configures CORS, sets up Axum router and shared `AppState`, handles logging, and wires requests to endpoints.
- [model.rs](file:///media/hzjnauary/Workspace/videodownloader/backend/src/model.rs): Holds the core domain models (`VideoInfo`, `StreamInfo`, and `ChannelVideo`) used for type safety and serialization across the boundary.
- [bulk.rs](file:///media/hzjnauary/Workspace/videodownloader/backend/src/bulk.rs): Manages concurrent download worker tasks using Tokio green threads. It retrieves raw streams, applies requested MP4 quality selection, optionally transcodes MP3 output through local GStreamer tools, and streams ZIP archive bytes back through a memory channel directly to the Axum HTTP response.
- [extract/](file:///media/hzjnauary/Workspace/videodownloader/backend/src/extract/mod.rs): Extraction engines for single video URLs.
  - [youtube.rs](file:///media/hzjnauary/Workspace/videodownloader/backend/src/extract/youtube.rs): Calls the YouTube InnerTube API (`/youtubei/v1/player`) via JSON POST using the `ANDROID` client context to obtain un-deciphered stream URLs.
  - [tiktok.rs](file:///media/hzjnauary/Workspace/videodownloader/backend/src/extract/tiktok.rs): Parses `SIGI_STATE` or `__NEXT_DATA__` from video HTML.
  - [facebook.rs](file:///media/hzjnauary/Workspace/videodownloader/backend/src/extract/facebook.rs): Scrapes page source for high/standard-definition direct MP4 links (`playable_url`, `playable_url_quality_hd`, etc.).
- [channel/](file:///media/hzjnauary/Workspace/videodownloader/backend/src/channel/mod.rs): Scrapers and collection crawlers. Implements pagination, cursor forwarding, continuation tokens, and raw HTML scraping for YouTube channel/playlist, TikTok profile, and Facebook page/profile videos.

### Frontend Structure (`frontend/`)

- [frontend/app/](file:///media/hzjnauary/Workspace/videodownloader/frontend/app/): Next.js App Router directory. The user interface uses native React hooks for stateful selection management, optional browser cookie input fields, and supports concurrent download streams.

---

## Architectural Principles & Boundary Rules

### 1. Parse-First Boundary Rule

Raw string inputs, queries, and payload parameters must be validated and parsed at the API boundary before entering core processing modules.

- API handlers parse incoming query arguments into strongly-typed parameters.
- Platform hosts are evaluated against explicit string patterns to reject unsupported URL formats early with a `400 Bad Request` status.

### 2. Stateless Forwarded Credentials

Authentication barriers on platforms like Facebook and TikTok are bypassed by allowing users to supply an optional provider cookie (`cookie` field).

- The cookie is forwarded in the request headers of outgoing HTTP calls to upstream platforms.
- The server is completely stateless and does not persist or cache these cookies.

### 3. ZIP Streaming (No Disk Buffer)

To keep memory consumption low and allow bulk downloading of large files, the backend streams the ZIP archive on the fly:

- Axum serves a chunked stream.
- A Tokio worker downloads files concurrently.
- Compressed bytes are written directly to the response writer, keeping disk usage zero.
