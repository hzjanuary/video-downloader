# Epic 04 Bulk Download Exec Plan

## Goal

Build a browser bulk selection flow and an Axum ZIP streaming endpoint for
selected video IDs.

## Scope

In scope:

- US-009 frontend bulk UI.
- US-010 backend `/api/download/bulk`.
- Checkbox selection and select-all state.
- Streaming ZIP response without temp files.
- Backend route and archive validation.

Out of scope:

- Background job queue.
- Persisted download history.
- Zip64 support.
- Per-file progress events.

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
2. Add backend bulk downloader and ZIP stream.
3. Add Axum route and CORS POST support.
4. Replace frontend first screen with bulk UI.
5. Validate Rust and Next.js.
6. Smoke test ZIP response.
7. Update Harness stories and trace.

## Stop Conditions

Pause for human confirmation if:

- The request contract must change away from selected IDs.
- Downloading requires credentials or browser-only provider sessions.
- Validation would require buffering complete large videos in memory.
