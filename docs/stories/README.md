# Story Packets

Stories represent discrete, testable work units. This directory contains story packets and historical validation records mapping functional epics to implementation files and proofs.

## Active Epics and Stories

### Base Slice Setup
- [US-001-frontend-nextjs.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/US-001-frontend-nextjs.md): Next.js app scaffolding and basic interface.
- [US-002-backend-axum.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/US-002-backend-axum.md): Axum REST API orchestration.
- [US-003-health-integration.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/US-003-health-integration.md): Cross-origin (CORS) health verification.

### Epic 2: Single Extraction ([epic-02-single-extraction](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-02-single-extraction/))
- [US-004-video-info-model.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-02-single-extraction/US-004-video-info-model.md): Schema models for streams and metadata.
- [US-005-youtube-parser.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-02-single-extraction/US-005-youtube-parser.md): YouTube InnerTube API fetcher.
- [US-006-tiktok-parser.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-02-single-extraction/US-006-tiktok-parser.md): TikTok SIGI_STATE HTML scraper.

### Epic 3: Channel Fetching ([epic-03-channel-fetching](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-03-channel-fetching/))
- [US-007-youtube-channel-fetching.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-03-channel-fetching/US-007-youtube-channel-fetching.md): LockupViewModel parser and continuation loops.
- [US-008-tiktok-profile-fetching.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-03-channel-fetching/US-008-tiktok-profile-fetching.md): Rehydration data model matching cursor requests.

### Epic 4: Bulk Download ([epic-04-bulk-download](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-04-bulk-download/))
- [US-009-frontend-bulk-ui.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-04-bulk-download/US-009-frontend-bulk-ui.md): Interactive selections dashboard.
- [US-010-backend-bulk-worker.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-04-bulk-download/US-010-backend-bulk-worker.md): In-memory ZIP writing and concurrent Tokio fetch tasks.

### Epic 5: Facebook Extraction ([epic-05-facebook-extraction](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-05-facebook-extraction/))
- [US-011-facebook-single-parser.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-05-facebook-extraction/US-011-facebook-single-parser.md): Scrapers parsing MP4 candidates in standard/high quality.
- [US-012-facebook-page-profile.md](file:///media/hzjnauary/Workspace/videodownloader/docs/stories/epic-05-facebook-extraction/US-012-facebook-page-profile.md): End-cursor crawler loops on Facebook Page video lists.

---

## Status Flow

Story statuses transition as follows:

```text
planned -> in_progress -> implemented
                              |
                              v
                           changed
                              |
                              v
                           retired
```
