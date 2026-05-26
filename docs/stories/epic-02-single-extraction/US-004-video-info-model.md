# US-004 VideoInfo Model

## Status

implemented

## Lane

high-risk

## Product Contract

The backend exposes a shared Rust metadata model for all single-video provider
parsers.

## Relevant Product Docs

- `docs/product/extraction.md`

## Acceptance Criteria

- `VideoInfo` includes platform, source URL, id, title, author, duration,
  thumbnail, and streams.
- `StreamInfo` includes URL, media traits, quality, dimensions, bitrate, and
  watermark marker.
- The model serializes to the API JSON shape.

## Evidence

- `cargo test --offline` passed with route tests asserting normalized JSON.
