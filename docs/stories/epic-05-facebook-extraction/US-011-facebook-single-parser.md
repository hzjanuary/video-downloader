# US-011 Facebook Single Parser

## Status

implemented

## Lane

high-risk

## Product Contract

Facebook Video/Reels URLs return normalized `VideoInfo` with at least one MP4
stream when the fetched HTML exposes a playable MP4 URL.

## Relevant Product Docs

- `docs/product/extraction.md`
- `docs/product/platform.md`

## Acceptance Criteria

- `/api/extract` accepts Facebook hosts.
- The Rust parser extracts MP4 stream URLs from Facebook HTML/JSON shapes.
- Optional cookie input can be forwarded to Facebook fetches.

## Design Notes

- API: `GET /api/extract?url=...&cookie=...`
- Domain rules: no cookie persistence.
- UI surfaces: optional cookie field.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | Facebook parser fixture tests |
| Integration | `/api/extract` Facebook fixture route test |
| E2E | Frontend typecheck/build |
| Platform | Harness matrix |
| Release | Not applicable |

## Harness Delta

Epic 05 story packet added.

## Evidence

- `cargo fmt --check`
- `cargo check --offline`
- `cargo test --offline`
- `npm run typecheck`
- `npm run build`
- `/api/extract` Facebook fixture route test returns normalized `platform:
  facebook` with MP4 stream URL.
- Live Facebook sample returned auth-wall HTML and `/api/extract` returned
  `422 no playable streams found`, matching the documented cookie-dependent
  provider limitation.
