# Validation

## Proof Strategy

Deterministic fixtures prove parser and route behavior. Live Facebook smoke is
best-effort because Facebook often blocks unauthenticated HTTP clients.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | Facebook MP4 fields, JSON-LD content URL, Page/Profile cursor listing |
| Integration | `/api/extract`, `/api/channel`, `/api/download/bulk` with existing ZIP flow |
| E2E | Frontend typecheck/build after cookie UI change |
| Platform | Harness matrix and route smoke |
| Performance | Existing streaming ZIP path unchanged |
| Logs/Audit | No persisted cookie or audit data |

## Fixtures

- Facebook single video HTML with `browser_native_hd_url`.
- Facebook JSON-LD `VideoObject` with `contentUrl`.
- Facebook Page/Profile initial page plus cursor continuation.

## Commands

```text
cargo fmt --check
cargo check --offline
cargo test --offline
npm run typecheck
npm run build
scripts/harness query matrix
```

## Acceptance Evidence

- `cargo fmt --check`: pass.
- `cargo check --offline`: pass.
- `cargo test --offline`: 25 passed.
- `npm run typecheck`: pass.
- `npm run build`: pass.
- Live `/api/extract` Facebook sample: `422 no playable streams found` because
  fetched HTML was auth-wall/login content without exposed MP4.
- Live `/api/download/bulk` smoke: `200 application/zip`, extracted ZIP entry
  contained backend health text.
