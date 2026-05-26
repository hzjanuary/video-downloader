# Epic 04 Bulk Download Design

## Domain Model

- Bulk request: `source_url` plus selected `ids`.
- ZIP item filename: sanitized ID with `.bin` extension.

## Application Flow

1. Frontend fetches channel/profile items from `/api/channel`.
2. Frontend stores selected IDs in a `Set`.
3. Frontend posts `{ source_url, ids }` to `/api/download/bulk`.
4. Backend deduplicates IDs and starts Tokio download tasks.
5. Each task sends byte chunks through a bounded channel.
6. ZIP writer streams entries to Axum response and writes central directory at
   the end.
7. Browser writes the response stream to a file when available, with Blob
   fallback.

## Interface Contract

- `GET /api/channel?url={link}` returns selectable video items.
- `POST /api/download/bulk` returns `application/zip`.

## Data Model

No database tables are added.

## UI / Platform Impact

The Next.js first screen is now the bulk download workflow.

## Observability

No durable job events are recorded yet. Errors use the existing JSON error
envelope before streaming starts.

## Alternatives Considered

1. Buffer all files before zipping: rejected because it violates the memory
   requirement.
2. Write temp files before response: rejected for this slice.
3. Pull `async-zip`: not present in the local cache; a minimal streaming ZIP
   writer was implemented instead.
