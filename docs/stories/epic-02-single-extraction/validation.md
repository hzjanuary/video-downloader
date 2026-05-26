# Epic 02 Single Extraction Validation

## Proof Strategy

Use deterministic unit and route tests with representative provider HTML
fixtures. Do not require live YouTube or TikTok responses for the first proof,
because those pages are volatile and network-dependent.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | Model serialization, provider detection, balanced JSON extraction, YouTube parser, TikTok SIGI parser, TikTok Next data parser |
| Integration | `/api/extract` route returns normalized YouTube and TikTok metadata with fixture-backed fetcher |
| E2E | Not required; no frontend flow in this slice |
| Platform | `cargo fmt --check`, `cargo check --offline`, `cargo test --offline` |
| Performance | Not required for first single-link slice |
| Logs/Audit | Not required; no product audit event |

## Fixtures

- YouTube HTML containing `ytInitialPlayerResponse`.
- TikTok HTML containing `SIGI_STATE`.
- TikTok HTML containing `__NEXT_DATA__`.

## Commands

```text
cargo fmt --check
cargo check --offline
cargo test --offline
```

## Acceptance Evidence

- `cargo fmt --check` passed.
- `cargo check --offline` passed.
- `cargo test --offline` passed with 9 tests.
