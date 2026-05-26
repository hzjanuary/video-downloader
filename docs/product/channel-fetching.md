# Channel Fetching Contract

## Scope

Channel fetching accepts one YouTube channel/user/handle/playlist URL or one
TikTok profile URL and returns a complete list of short video metadata collected
through provider pagination.

## API

- Route: `GET /api/channel?url={link}`
- Response: JSON array.
- Item shape:

```json
{
  "id": "video-id",
  "title": "Video title",
  "thumbnail_url": "https://image.example/thumb.jpg"
}
```

## Supported Inputs

- YouTube:
  - `/channel/...`
  - `/c/...`
  - `/user/...`
  - `/@handle/...`
  - `/playlist?list=...`
- TikTok:
  - `/@username`

Unsupported URLs return `400`.

## Provider Pagination

- YouTube: parse `ytInitialData`, collect `videoRenderer`,
  `gridVideoRenderer`, and `playlistVideoRenderer` items, then follow
  `continuationCommand.token` through the YouTube `youtubei/v1/browse`
  continuation endpoint.
- TikTok: parse `SIGI_STATE` or `__NEXT_DATA__`, collect `ItemModule` and
  feed items, then follow `cursor`, `maxCursor`, or `max_cursor` while
  `hasMore` / `has_more` is true.

## Rate-Limit Handling

The backend uses a browser-like User-Agent for provider requests, retries
failed provider fetches, and adds delay between pagination calls.

## Validation

Repeatable proof uses deterministic fixtures:

- YouTube test channel: initial page plus two continuation pages, five total
  videos.
- TikTok test profile: initial page plus two cursor pages, five total videos.
