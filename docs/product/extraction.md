# Extraction Contract

## Scope

Single extraction accepts one public YouTube or TikTok video URL and returns
normalized metadata. The backend implements the fetch and parser logic directly
in Rust without using `yt-dlp` or provider wrapper modules.

## API

- Route: `GET /api/extract?url={link}`
- Supported hosts:
  - `youtube.com`, `*.youtube.com`, `youtu.be`
  - `tiktok.com`, `*.tiktok.com`
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

- YouTube: fetch HTML with `reqwest`, locate `ytInitialPlayerResponse` with
  `regex`, extract the balanced JSON object, parse it with `serde_json`, and
  read `videoDetails` plus `streamingData.formats` and
  `streamingData.adaptiveFormats`.
- TikTok: fetch HTML with `reqwest`, locate `SIGI_STATE` or `__NEXT_DATA__`
  script JSON with `regex`, parse with `serde_json`, and read item/video fields.
  `playAddr` and bitrate `PlayAddr.UrlList` entries are treated as no-watermark
  stream candidates; `downloadAddr` is a watermark fallback.

## Validation

Parser and route tests use controlled HTML fixtures for YouTube
`ytInitialPlayerResponse`, TikTok `SIGI_STATE`, and TikTok `__NEXT_DATA__`.
Live provider HTML can change without notice, so deterministic fixture tests are
the required proof for the first implementation slice.
