# Extraction Contract

## Scope

Single extraction accepts one public YouTube, TikTok, or Facebook video URL and returns
normalized metadata. The backend implements the fetch and parser logic directly
in Rust without using `yt-dlp` or provider wrapper modules.

## API

- Route: `GET /api/extract?url={link}`
- Supported hosts:
  - `youtube.com`, `*.youtube.com`, `youtu.be`
  - `tiktok.com`, `*.tiktok.com`
  - `facebook.com`, `*.facebook.com`, `fb.watch`, `*.fb.watch`
- Optional query:
  - `cookie`: provider cookie string forwarded only to the upstream fetch. This
    is intended for local Facebook auth-wall bypass when public HTML is blocked.
- Unsupported hosts return `400`.
- Provider fetch failures return `502`.
- Missing or invalid provider JSON returns `422`.

## Normalized Response

```json
{
  "platform": "youtube",
  "source_url": "https://www.youtube.com/watch?v=abc123",
  "id": "abc123",
  "title": "Example video",
  "author": "Example channel",
  "duration_seconds": 61,
  "thumbnail_url": "https://img.example/large.jpg",
  "streams": [
    {
      "url": "https://video.example/itag18.mp4",
      "mime_type": "video/mp4",
      "quality": "360p",
      "width": 640,
      "height": 360,
      "bitrate": 400000,
      "has_audio": true,
      "has_video": true,
      "watermark": false
    }
  ]
}
```

## Provider Parsing

- YouTube: call the InnerTube player endpoint
  `https://www.youtube.com/youtubei/v1/player` with an Android client context,
  parse the JSON response with `serde_json`, and read `videoDetails` plus
  direct stream URLs from `streamingData.formats` and
  `streamingData.adaptiveFormats`. YouTube single-video extraction does not
  parse `ytInitialPlayerResponse` from raw watch-page HTML and does not run a
  local signature decipher routine. The backend pins a current working Android
  client version for this request.
- TikTok: fetch HTML with `reqwest`, locate `SIGI_STATE` or `__NEXT_DATA__`
  script JSON with `regex`, parse with `serde_json`, and read item/video fields.
  `playAddr` and bitrate `PlayAddr.UrlList` entries are treated as no-watermark
  stream candidates; `downloadAddr` is a watermark fallback.
- Facebook: fetch HTML with a browser-like User-Agent and optional cookie,
  locate MP4 candidates in metadata, JSON-LD, and embedded JSON string fields
  such as `playable_url`, `playable_url_quality_hd`,
  `browser_native_hd_url`, `browser_native_sd_url`, `hd_src`, and `sd_src`.
  The parser normalizes those MP4 URLs into `StreamInfo`.

## Validation

Parser and route tests use controlled InnerTube JSON fixtures for YouTube,
HTML fixtures for TikTok `SIGI_STATE` / `__NEXT_DATA__`, and Facebook embedded
MP4 fields / JSON-LD. Live provider behavior can change without notice, and
Facebook often requires authenticated cookies, so deterministic fixture tests
remain required proof.
