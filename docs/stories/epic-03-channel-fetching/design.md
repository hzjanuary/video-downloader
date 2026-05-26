# Epic 03 Channel Fetching Design

## Domain Model

- `ChannelVideo`: short metadata item with `id`, `title`, and
  `thumbnail_url`.

## Application Flow

1. Parse `url` from `/api/channel`.
2. Accept only supported YouTube channel/playlist paths or TikTok profile paths.
3. Fetch initial HTML with a browser-like User-Agent.
4. Parse embedded provider JSON.
5. Collect video metadata.
6. Follow YouTube continuation tokens or TikTok cursors with retry and delay.
7. Return the complete deduplicated array.

## Interface Contract

- `200`: JSON array of `ChannelVideo`.
- `400`: unsupported URL.
- `502`: provider fetch failure.
- `422`: missing provider JSON, invalid provider JSON, missing pagination
  fields, or no videos.

## Data Model

No database tables are added. Channel fetching is stateless.

## UI / Platform Impact

No frontend change in this slice.

## Observability

Errors use the existing JSON `{ "error": "..." }` envelope. Request logging is
still future work.

## Alternatives Considered

1. Use provider wrapper libraries: rejected by earlier extraction constraints.
2. Stop at first page: rejected because Epic 03 requires pagination.
