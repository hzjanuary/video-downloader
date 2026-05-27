# Product Contracts

This directory contains the living product contracts and domain specifications for the Video Downloader platform.

## Active Contracts

- [extraction.md](file:///media/hzjnauary/Workspace/videodownloader/docs/product/extraction.md): Rules for parsing and extracting metadata and direct stream URLs for single video pages (YouTube InnerTube API, TikTok SIGI_STATE, Facebook HTML).
- [channel-fetching.md](file:///media/hzjnauary/Workspace/videodownloader/docs/product/channel-fetching.md): Details for paginated channel crawlers (YouTube continuation token traversal, TikTok rehydration parser, Facebook profile cursors).
- [bulk-download.md](file:///media/hzjnauary/Workspace/videodownloader/docs/product/bulk-download.md): Rules for selecting, packing, and streaming concurrent media downloads as a single, on-the-fly ZIP archive.
- [platform.md](file:///media/hzjnauary/Workspace/videodownloader/docs/product/platform.md): System integration points (health API, origin-based CORS policies, and provider credentials forwarding).

## Update Rule

When product behavior changes:

1. Update the affected product contract file in this directory.
2. Update or create the corresponding story packet under `docs/stories/`.
3. Update the durable proof status with `scripts/harness story update`.
4. Record an architecture decision under `docs/decisions/` if the change alters high-level system properties, security, or technology stack choices.
