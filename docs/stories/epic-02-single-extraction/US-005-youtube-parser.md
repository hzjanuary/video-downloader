# US-005 YouTube Parser

## Status

implemented

## Lane

high-risk

## Product Contract

The backend fetches YouTube HTML and parses `ytInitialPlayerResponse` directly
to produce normalized `VideoInfo`.

## Relevant Product Docs

- `docs/product/extraction.md`

## Acceptance Criteria

- Uses `reqwest` for HTML fetch in the extractor path.
- Uses `regex` to locate `ytInitialPlayerResponse`.
- Uses `serde_json` to parse provider JSON.
- Extracts video details and stream URLs from `streamingData`.

## Evidence

- `cargo test --offline` passed with YouTube parser and route fixture coverage.
