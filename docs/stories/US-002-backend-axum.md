# US-002 Rust Axum Backend

## Status

implemented

## Lane

normal

## Product Contract

Create a Rust Axum backend in `backend` that runs on port `8080`.

## Relevant Product Docs

- `docs/product/platform.md`

## Acceptance Criteria

- `backend` contains a Rust Cargo project using Axum and Tokio.
- `cargo run` starts the API on port `8080`.
- Backend tests validate the health route behavior.

## Design Notes

- API surface: `backend/src/main.rs`.
- Runtime port: `8080`.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | Route-level test for `/api/health`. |
| Integration | Smoke request to `http://localhost:8080/api/health`. |
| E2E | Covered by US-003 web integration. |
| Platform | `cargo test` and `cargo run` smoke. |
| Release | Not required for local setup slice. |

## Harness Delta

Created story packet for the initial backend platform setup.

## Evidence

- `cargo test --offline` passed with route and CORS coverage.
- `cargo check --offline` passed.
- `cargo run` started the API on `http://localhost:8080` during smoke
  validation.
- `curl -i -H "Origin: http://localhost:3000" http://localhost:8080/api/health`
  returned `HTTP/1.1 200 OK`, `access-control-allow-origin:
  http://localhost:3000`, and `OK: backend is healthy`.
