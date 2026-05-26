# US-008 TikTok Profile Fetching

## Status

implemented

## Lane

high-risk

## Product Contract

The backend accepts TikTok profile URLs and returns all available short video
metadata by following feed cursors.

## Relevant Product Docs

- `docs/product/channel-fetching.md`

## Acceptance Criteria

- Detects TikTok profile URLs in `/@username` form.
- Parses `SIGI_STATE` or `__NEXT_DATA__`.
- Collects video IDs, titles, and thumbnail URLs.
- Follows `cursor`, `maxCursor`, or `max_cursor` while `hasMore` is true.
- Exposes results through `/api/channel`.

## Evidence

- `cargo test --offline` passed.
- TikTok fixture route test returned five videos across initial page plus two
  cursor pages.
