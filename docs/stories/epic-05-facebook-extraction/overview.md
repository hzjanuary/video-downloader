# Overview

## Current Behavior

The app supports YouTube and TikTok extraction/listing. Facebook URLs are
rejected as unsupported.

## Target Behavior

The app accepts Facebook video/Reels URLs for single extraction and Facebook
Page/Profile video surfaces for collection listing. A provider cookie can be
supplied when Facebook returns an auth-wall.

## Affected Users

- Browser user downloading videos from supported provider URLs.
- Operator validating provider parser behavior.

## Affected Product Docs

- `docs/product/extraction.md`
- `docs/product/channel-fetching.md`
- `docs/product/platform.md`

## Non-Goals

- Persisting provider cookies.
- Implementing Facebook login.
- Guaranteeing live Facebook access without a user-provided cookie.
