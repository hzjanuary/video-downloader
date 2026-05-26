# Epic 02 Single Extraction Exec Plan

## Goal

Implement single-link metadata extraction for YouTube and TikTok with a shared
Rust response model and a backend API route.

## Scope

In scope:

- US-004 normalized `VideoInfo` and `StreamInfo`.
- US-005 YouTube HTML/JSON parser.
- US-006 TikTok HTML/JSON parser.
- `GET /api/extract?url={link}` in Axum.
- Deterministic parser and route tests.

Out of scope:

- Downloading files.
- Batch extraction.
- Frontend extraction UI.
- Live provider availability guarantees.

## Risk Classification

Risk flags:

- External systems.
- Public contracts.
- Data model.
- Weak proof.

Hard gates:

- External provider behavior.

## Work Phases

1. Record intake and inspect existing backend.
2. Add normalized model and extractor modules.
3. Add YouTube and TikTok parsers with fixtures.
4. Add `/api/extract` and route tests.
5. Run backend validation.
6. Update product docs and Harness status.

## Stop Conditions

Pause for human confirmation if:

- The API response shape needs to change incompatibly.
- The implementation would require provider wrapper tools.
- Validation would need to depend only on live provider pages.
