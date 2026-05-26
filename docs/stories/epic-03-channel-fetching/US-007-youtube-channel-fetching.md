# US-007 YouTube Channel Fetching

## Status

implemented

## Lane

high-risk

## Product Contract

The backend accepts YouTube channel/user/handle/playlist URLs and returns all
available short video metadata by following continuation tokens.

## Relevant Product Docs

- `docs/product/channel-fetching.md`

## Acceptance Criteria

- Detects supported YouTube channel and playlist URL shapes.
- Parses `ytInitialData` from HTML.
- Collects video IDs, titles, and thumbnail URLs.
- Follows `continuationCommand.token` pages.
- Exposes results through `/api/channel`.

## Evidence

- `cargo test --offline` passed.
- YouTube fixture route test returned five videos across initial page plus two
  continuation pages.
