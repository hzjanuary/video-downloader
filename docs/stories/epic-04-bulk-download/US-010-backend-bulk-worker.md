# US-010 Backend Bulk Worker

## Status

implemented

## Lane

high-risk

## Product Contract

The backend accepts selected video IDs, downloads them concurrently, and streams
one ZIP archive to the client without buffering the full archive in RAM or
writing temp files.

## Relevant Product Docs

- `docs/product/bulk-download.md`

## Acceptance Criteria

- `POST /api/download/bulk` accepts an ID array.
- Tokio tasks download selected IDs concurrently.
- ZIP bytes stream directly through Axum.
- Backend does not create temp files.
- Archive validation proves the returned file opens and contains expected
  content.

## Evidence

- `cargo test --offline` passed with ZIP route coverage.
- HTTP smoke returned `application/zip` with chunked transfer encoding.
- `unzip -l` listed the archive and `unzip -p` returned
  `OK: backend is healthy`.
