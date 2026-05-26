# Epic 04 Bulk Download Overview

## Current Behavior

The app can list channel/profile videos through the API, but the frontend does
not expose a selection workflow and the backend cannot return a bulk archive.

## Target Behavior

The frontend renders a bulk workflow with URL input, video list, row checkboxes,
`Chọn tất cả`, and a download button. The backend accepts selected IDs at
`POST /api/download/bulk` and streams a ZIP archive back to the client.

## Affected Users

- Browser users downloading multiple videos from a channel/profile result.
- API consumers posting selected IDs directly.

## Affected Product Docs

- `docs/product/bulk-download.md`
- `docs/product/channel-fetching.md`

## Non-Goals

- Download progress per individual file.
- Persistent download jobs.
- Temporary file storage.
- Zip64 archives over 4GB in this slice.
