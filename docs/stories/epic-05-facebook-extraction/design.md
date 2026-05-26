# Design

## Domain Model

`Platform` gains `facebook`. Existing `VideoInfo`, `StreamInfo`, and
`ChannelVideo` remain the response contracts.

## Application Flow

Single extraction fetches provider HTML with a browser-like User-Agent and an
optional cookie. The Facebook parser extracts MP4 stream candidates from
metadata, JSON-LD, and embedded JSON string fields.

Collection listing fetches Page/Profile HTML with the same optional cookie,
collects Facebook video IDs from links and embedded Video/Reel JSON nodes, and
follows visible next-page URLs or `page_info.end_cursor` fixture flows.

Bulk download resolves Facebook IDs to `https://www.facebook.com/watch/?v={id}`
and reuses the existing extractor to find a stream URL.

## Interface Contract

- `GET /api/extract?url=...&cookie=...`
- `GET /api/channel?url=...&cookie=...`
- `POST /api/download/bulk` accepts optional `cookie`.

The cookie is forwarded to upstream provider requests only.

## Data Model

No persistence changes.

## UI / Platform Impact

The frontend exposes an optional provider cookie field and includes it in fetch
and bulk download calls.

## Observability

No new persistent logs. Provider failures continue through existing API error
responses.

## Alternatives Considered

1. Use `yt-dlp`: rejected by project contract requiring self-written parsers.
2. Store cookies server-side: rejected because the app has no auth/session model.
