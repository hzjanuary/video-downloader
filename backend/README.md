# Backend

Rust Axum API for the video downloader platform setup slice.

## Commands

```bash
cargo run
cargo test
```

The server listens on `http://localhost:8080`.

## API

- `GET /api/health` returns `OK: backend is healthy`.
- `GET /api/extract?url={link}` fetches YouTube or TikTok HTML and returns
  normalized video metadata as JSON.
- `GET /api/channel?url={link}` fetches a YouTube channel/playlist or TikTok
  profile and returns a JSON array of short video metadata.
- CORS allows the Next.js dev origin `http://localhost:3000`.
