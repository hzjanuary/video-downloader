# US-005 YouTube Parser

## Status

implemented

## Lane

high-risk

## Product Contract

The backend calls the YouTube InnerTube player endpoint with an Android client
context and parses the JSON response directly to produce normalized
`VideoInfo`.

## Relevant Product Docs

- `docs/product/extraction.md`

## Acceptance Criteria

- Uses `reqwest` to POST to `https://www.youtube.com/youtubei/v1/player`.
- Sends `context.client.clientName = "ANDROID"` with the current working
  Android client version used by the backend.
- Uses `serde_json` to parse provider JSON.
- Extracts video details and direct stream URLs from `streamingData`.
- Does not run local YouTube signature decipher logic.

## Evidence

- `cargo test --offline` passed with YouTube InnerTube parser and route fixture
  coverage.
