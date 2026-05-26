# Epic 03 Channel Fetching Overview

## Current Behavior

The backend can extract metadata from a single video URL, but it cannot list
videos from a YouTube channel/playlist or TikTok profile.

## Target Behavior

The backend exposes `GET /api/channel?url={link}`. It returns a JSON array of
short video metadata and follows provider pagination until the feed is complete
or the safety page limit is reached.

## Affected Users

- API consumers requesting channel/profile video lists.
- Future browser users who will paste channel/profile URLs.

## Affected Product Docs

- `docs/product/channel-fetching.md`
- `docs/product/extraction.md`

## Non-Goals

- Downloading listed videos.
- Frontend channel UI.
- Persisting channel results.
- Guaranteeing live provider availability in tests.
