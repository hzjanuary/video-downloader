# Documentation Map

This directory holds the project harness and any product contract derived from a
future user-provided spec.

## Main Files

- `HARNESS.md`: how humans and agents collaborate.
- `FEATURE_INTAKE.md`: how prompts become tiny, normal, or high-risk work.
- `ARCHITECTURE.md`: architecture discovery and boundary rules.
- `TEST_MATRIX.md`: legacy proof map; current proof status is queried with
  `scripts/harness query matrix`.
- `HARNESS_BACKLOG.md`: legacy improvement list; current improvement records
  are stored with `scripts/harness backlog`.
- `GLOSSARY.md`: shared terms.

## Folders

- `product/`: current product truth and API/platform contracts.
- `stories/`: epic packets and historical validation records for US-001 through US-012.
- `decisions/`: durable decisions and tradeoffs.
- `templates/`: reusable spec-intake, story, plan, decision, and validation formats.

## Current State

The project is fully implemented (Harness v1). Next.js frontend, Rust Axum backend, and all 12 user stories (US-001 through US-012) are fully functional, verified by automated unit/integration tests and Harness verification CLI commands.
