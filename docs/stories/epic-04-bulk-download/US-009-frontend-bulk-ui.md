# US-009 Frontend Bulk UI

## Status

implemented

## Lane

high-risk

## Product Contract

The Next.js app lets a user enter a link, fetch videos, select individual or all
videos, and submit the selected ID array for bulk download.

## Relevant Product Docs

- `docs/product/bulk-download.md`

## Acceptance Criteria

- Form accepts a channel/profile/playlist link.
- UI calls `/api/channel`.
- UI renders the video list.
- Checkboxes support one, many, and `Chọn tất cả`.
- Download action sends selected IDs to `/api/download/bulk`.
- File save path uses response stream where supported and Blob fallback.

## Evidence

- `npm run typecheck` passed.
- `npm run build` passed.
- Dev server smoke returned HTML containing `Bulk Download` and `Chọn tất cả`.
