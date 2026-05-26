# Epic 04 Bulk Download Validation

## Proof Strategy

Use deterministic route tests for selected IDs and a local HTTP smoke test that
streams a real ZIP response from Axum.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | ID deduplication and ZIP entry creation |
| Integration | `/api/download/bulk` route returns ZIP with selected fixture files |
| E2E | Next.js page renders bulk workflow controls |
| Platform | `cargo fmt --check`, `cargo check --offline`, `cargo test --offline`, `npm run typecheck`, `npm run build` |
| Performance | Bounded channels and streamed response avoid large RAM buffers |
| Logs/Audit | Not required |

## Fixtures

- Fixture download IDs `yt001` and `yt002`.
- Local smoke ID `http://localhost:8080/api/health`.

## Commands

```text
cargo fmt --check
cargo check --offline
cargo test --offline
npm run typecheck
npm run build
curl -sS -X POST http://localhost:8080/api/download/bulk ...
unzip -l /tmp/videodownloader-bulk-smoke.zip
unzip -p /tmp/videodownloader-bulk-smoke.zip http___localhost_8080_api_health.bin
```

## Acceptance Evidence

- `cargo fmt --check` passed.
- `cargo check --offline` passed.
- `cargo test --offline` passed with 17 tests.
- `npm run typecheck` passed.
- `npm run build` passed.
- Smoke ZIP listed one entry and `unzip -p` returned `OK: backend is healthy`.
