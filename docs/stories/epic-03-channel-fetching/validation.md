# Epic 03 Channel Fetching Validation

## Proof Strategy

Provider pages and feed APIs change frequently, so deterministic fixtures prove
pagination behavior. The live implementation still uses `reqwest`, fake
User-Agent, retry, and delay for provider calls.

## Test Plan

| Layer | Cases |
| --- | --- |
| Unit | YouTube channel URL detection, YouTube continuation traversal, TikTok profile cursor traversal |
| Integration | `/api/channel` route returns all fixture videos for YouTube and TikTok |
| E2E | Not required; no frontend channel flow in this slice |
| Platform | `cargo fmt --check`, `cargo check --offline`, `cargo test --offline` |
| Performance | Pagination has retry, delay, deduplication, and a page safety limit |
| Logs/Audit | Not required |

## Fixtures

- YouTube channel fixture: initial page plus `YT_CONT_1` and `YT_CONT_2`,
  returning five unique videos.
- TikTok profile fixture: initial page plus cursor `20` and cursor `40`,
  returning five unique videos.

## Commands

```text
cargo fmt --check
cargo check --offline
cargo test --offline
```

## Acceptance Evidence

- `cargo fmt --check` passed.
- `cargo check --offline` passed.
- `cargo test --offline` passed with 14 tests.
- Route tests confirmed both test feeds return exactly five videos.
