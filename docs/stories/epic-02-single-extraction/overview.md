# Epic 02 Single Extraction Overview

## Current Behavior

The backend exposes health only. There is no normalized video metadata model and
no extraction endpoint.

## Target Behavior

The backend exposes `GET /api/extract?url={link}` for YouTube and TikTok single
video URLs. It fetches provider HTML, parses embedded JSON directly, and returns
the shared `VideoInfo` response shape.

## Affected Users

- Browser users who submit a single video URL.
- API consumers calling the backend directly.

## Affected Product Docs

- `docs/product/extraction.md`
- `docs/product/platform.md`

## Non-Goals

- Playlist, channel, profile, or batch extraction.
- Download execution.
- Signature deciphering or DRM bypass.
- Provider wrapper tools such as `yt-dlp`.
