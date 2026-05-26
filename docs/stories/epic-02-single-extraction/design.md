# Epic 02 Single Extraction Design

## Domain Model

- `VideoInfo`: normalized metadata for all providers.
- `StreamInfo`: one candidate media stream URL plus media traits.
- `Platform`: `youtube` or `tiktok`.

## Application Flow

1. Parse `url` from `/api/extract`.
2. Reject unsupported hosts before fetching.
3. Fetch provider HTML with `reqwest`.
4. Route HTML to the provider parser.
5. Return `VideoInfo` as JSON or a typed JSON error.

## Interface Contract

- `GET /api/extract?url={link}` returns `200` with normalized metadata.
- `400` rejects unsupported hosts.
- `502` reports provider fetch failures.
- `422` reports parser failures or missing streams.

## Data Model

No database tables are added. Extraction is stateless.

## UI / Platform Impact

No frontend change in this slice.

## Observability

Errors are returned in a JSON `{ "error": "..." }` envelope. Request logging is
still future work from the architecture observability contract.

## Alternatives Considered

1. Use a provider wrapper tool: rejected by the requirement.
2. Add a general-purpose HTML parser: rejected for this slice because the spec
   asks for custom HTML scraping plus `regex` and `serde_json`.
