# Exec Plan

## Goal

Implement Epic 05: Facebook single-video MP4 extraction and Page/Profile video
listing integrated into existing backend routes and frontend download flow.

## Scope

In scope:

- Rust parser for Facebook Video/Reels MP4 fields.
- Facebook Page/Profile list crawler with pagination hooks.
- Optional cookie forwarding from UI to backend provider requests.
- Harness stories US-011 and US-012.

Out of scope:

- Facebook login automation.
- Persisted provider credentials.
- Browser execution or JavaScript challenge solving.

## Risk Classification

Risk flags:

- External systems.
- Public contracts.
- Existing behavior.
- Weak proof.

Hard gates:

- External provider behavior.

## Work Phases

1. Discovery.
2. Design.
3. Validation planning.
4. Implementation.
5. Verification.
6. Harness update.

## Stop Conditions

Pause for human confirmation if:

- A provider cookie must be stored.
- Route response shapes need to change.
- Validation requirements need to be weakened below fixture proof.
