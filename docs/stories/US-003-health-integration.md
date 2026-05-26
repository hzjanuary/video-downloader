# US-003 Health API Integration

## Status

implemented

## Lane

normal

## Product Contract

Expose backend health text at `/api/health`, allow the frontend origin through
CORS, and render the returned text in the Next.js web app.

## Relevant Product Docs

- `docs/product/platform.md`

## Acceptance Criteria

- Backend route `GET /api/health` returns `OK: backend is healthy`.
- Backend CORS allows `http://localhost:3000`.
- Frontend renders the exact backend health text.

## Design Notes

- API: `GET /api/health`.
- CORS: `tower-http` `CorsLayer`.
- Web integration: server-side `fetch` in the App Router page.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | Backend route test includes CORS header and response text. |
| Integration | `curl` health endpoint while backend is running. |
| E2E | `curl` frontend while both servers are running and verify health text. |
| Platform | Next.js and Cargo build/test commands. |
| Release | Not required for local setup slice. |

## Harness Delta

Created story packet for the health-check integration slice.

## Evidence

- `cargo test --offline` passed with `/api/health` response text and CORS
  assertion.
- Backend smoke request returned `OK: backend is healthy`.
- With backend and frontend dev servers running, `curl -sS http://localhost:3000`
  returned HTML containing `OK: backend is healthy`.
