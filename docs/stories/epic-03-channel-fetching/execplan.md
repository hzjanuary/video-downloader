# Epic 03 Channel Fetching Exec Plan

## Goal

Implement channel/profile video listing for YouTube and TikTok behind
`GET /api/channel?url={link}`.

## Scope

In scope:

- US-007 YouTube channel/user/playlist parsing and continuation pagination.
- US-008 TikTok profile parsing and cursor pagination.
- Short metadata JSON response.
- Retry, delay, and browser-like User-Agent for provider requests.
- Deterministic multi-page fixtures.

Out of scope:

- Frontend channel UI.
- File downloads.
- Data persistence.
- Playlist item enrichment beyond short metadata.

## Risk Classification

Risk flags:

- External systems.
- Public contracts.
- Data model.
- Weak proof.

Hard gates:

- External provider behavior.

## Work Phases

1. Record Harness intake.
2. Add `ChannelVideo` model.
3. Add channel fetcher and provider parsers.
4. Add `/api/channel` route.
5. Validate pagination with fixtures.
6. Update Harness stories and trace.

## Stop Conditions

Pause for human confirmation if:

- Provider pagination requires credentials or browser automation.
- API response shape needs incompatible expansion.
- Validation would depend only on live provider pages.
