# Channel Fetching Contract

## Scope

Channel fetching accepts one YouTube channel/user/handle/playlist URL, one
TikTok profile URL, or one Facebook Page/Profile videos URL and returns a list
of short video metadata collected through provider pagination when the provider
HTML exposes continuation data.

## API

- Route: `GET /api/channel?url={link}`
- Optional query:
  - `cookie`: provider cookie string forwarded only to upstream fetches. This is
    intended for local auth-wall bypass when public HTML is blocked. TikTok
    profile pagination can require a fresh browser cookie containing
    `s_v_web_id`.
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
- Facebook:
  - Page/Profile video surfaces on `facebook.com`, `*.facebook.com`
  - Reels/watch collections when the HTML exposes video links

Unsupported URLs return `400`.

## Provider Pagination

- YouTube: parse `ytInitialData`, collect `videoRenderer`,
  `gridVideoRenderer`, `playlistVideoRenderer`, and current
  `lockupViewModel` channel tiles, then follow
  `continuationCommand.token` through the YouTube `youtubei/v1/browse`
  continuation endpoint.
- TikTok: parse `SIGI_STATE`, `__NEXT_DATA__`, or
  `__UNIVERSAL_DATA_FOR_REHYDRATION__`, collect `ItemModule` and feed items,
  then follow `cursor`, `maxCursor`, or `max_cursor` while `hasMore` /
  `has_more` is true. When TikTok serves profile hydration with only
  `secUid` and an empty item list, the backend starts the profile feed cursor
  at `0`. If the caller supplies a cookie with `s_v_web_id`, cursor pagination
  uses TikTok's mobile feed endpoint with that verifier value.
- Facebook: parse Page/Profile HTML for video IDs from `/watch/?v=...`,
  `/videos/...`, `/reel/...`, and embedded JSON objects typed as Video/Reel.
  Pagination follows visible next-page links when present, or embedded
  `page_info.has_next_page` / `end_cursor` values for deterministic cursor
  flows.

## Rate-Limit Handling

The backend uses a browser-like User-Agent for provider requests, retries
failed provider fetches, and adds delay between pagination calls.

## Validation

Repeatable proof uses deterministic fixtures:

- YouTube test channel: initial page plus two continuation pages, five total
  videos, plus a separate `lockupViewModel` tile fixture.
- TikTok test profile: initial page plus two cursor pages, five total videos.
  Universal hydration with an empty item list is covered by a cursor `0`
  fixture.
- Facebook test page: initial Page/Profile video JSON plus one cursor page,
  three total videos.
