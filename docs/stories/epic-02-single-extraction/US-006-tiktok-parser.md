# US-006 TikTok Parser

## Status

implemented

## Lane

high-risk

## Product Contract

The backend fetches TikTok HTML and parses `SIGI_STATE` or `__NEXT_DATA__`
directly to produce normalized `VideoInfo` with no-watermark stream candidates.

## Relevant Product Docs

- `docs/product/extraction.md`

## Acceptance Criteria

- Uses `reqwest` for HTML fetch in the extractor path.
- Uses `regex` to locate `SIGI_STATE` or `__NEXT_DATA__`.
- Uses `serde_json` to parse provider JSON.
- Extracts metadata and no-watermark stream candidates from TikTok video state.

## Evidence

- `cargo test --offline` passed with TikTok `SIGI_STATE`, TikTok
  `__NEXT_DATA__`, and route fixture coverage.
