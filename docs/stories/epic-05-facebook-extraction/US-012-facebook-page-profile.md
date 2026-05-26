# US-012 Facebook Page/Profile Listing

## Status

implemented

## Lane

high-risk

## Product Contract

Facebook Page/Profile video surfaces return an array of `ChannelVideo` items
when the fetched HTML exposes video IDs and pagination data.

## Relevant Product Docs

- `docs/product/channel-fetching.md`
- `docs/product/platform.md`

## Acceptance Criteria

- `/api/channel` accepts Facebook Page/Profile URLs.
- The crawler collects Facebook video IDs from links and embedded Video/Reel JSON.
- The crawler follows next-page links or cursor continuation data.

## Design Notes

- API: `GET /api/channel?url=...&cookie=...`
- Domain rules: no cookie persistence.
- UI surfaces: optional cookie field passed through fetch and bulk download.

## Validation

| Layer | Expected proof |
| --- | --- |
| Unit | Facebook cursor pagination fixture test |
| Integration | `/api/channel` Facebook fixture route test |
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
- `/api/channel` Facebook fixture route test follows one cursor page and
  returns three `ChannelVideo` items.
- Live bulk download smoke returned `200 application/zip`; extracted ZIP entry
  contained `OK: backend is healthy`.
