# Test Matrix

This matrix maps project user stories and product behavior to proof. All stories are tracked inside the durable layer (`harness.db`) and are queried dynamically with:

```bash
scripts/harness query matrix
```

## Matrix Summary

| Story | Title | Unit | Integ | E2E | Platform | Status | Evidence / Validation Command |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **US-001** | Next.js frontend | no | no | yes | yes | implemented | `npm install; npm run typecheck; npm run build; npm run dev` smoke on `http://localhost:3000` |
| **US-002** | Rust Axum backend | yes | yes | no | yes | implemented | `cargo test --offline; cargo check --offline; cargo run;` curl backend health returned 200 and OK text |
| **US-003** | Health API web integration | yes | yes | yes | yes | implemented | backend CORS curl returned access-control-allow-origin `http://localhost:3000`; frontend HTML contained OK: backend is healthy |
| **US-004** | Normalized VideoInfo model | yes | yes | no | yes | implemented | `cargo test --offline` route assertions for normalized JSON; `cargo check --offline; cargo fmt --check` |
| **US-005** | YouTube InnerTube extractor | yes | yes | yes | yes | implemented | `cargo fmt --check; cargo check --offline; cargo test --offline` 26 passed; `/api/extract` live YouTube `mXEGebUXQRg` uses Android InnerTube player API and returned 200 with 26 direct stream URLs; old HTML Regex/signature decipher path removed from `backend/src/extract/youtube.rs` |
| **US-006** | TikTok HTML JSON parser | yes | yes | no | yes | implemented | `cargo test --offline` TikTok `SIGI_STATE` and `__NEXT_DATA__` fixtures parse no-watermark streams and `/api/extract` returns TikTok metadata JSON |
| **US-007** | YouTube channel and playlist fetching | yes | yes | no | yes | implemented | `cargo test --offline; cargo check --offline;` live `/api/channel` YouTube `@YouTube/videos` returned 200 with 183 metadata items after `lockupViewModel` parser fix |
| **US-008** | TikTok profile fetching | yes | yes | yes | yes | implemented | `cargo fmt --check; cargo check --offline; cargo test --offline` 25 passed; `npm run typecheck`; TikTok channel parser supports `__UNIVERSAL_DATA_FOR_REHYDRATION__`, cursor 0, forwarded cookie, and `m.tiktok` cursor path with `s_v_web_id`; live `@tiktok` remains provider verification-gated without fresh browser cookie |
| **US-009** | Frontend bulk selection UI | yes | yes | yes | yes | implemented | `npm run typecheck; npm run build;` Next.js dev smoke returned Bulk Download and Chọn tất cả; UI posts selected ID array to `/api/download/bulk` |
| **US-010** | Backend bulk zip worker | yes | yes | yes | yes | implemented | `cargo fmt --check; cargo check --offline; cargo test --offline` 26 passed; `npm run typecheck`; live YouTube single-video bulk download for `mXEGebUXQRg` returned 200 application/zip 10810144 bytes; unzip listed `mXEGebUXQRg.bin` length 10810000 without 403 |
| **US-011** | Facebook single video extraction | yes | yes | yes | yes | implemented | `cargo fmt --check; cargo check --offline; cargo test --offline` 25 passed; `npm run typecheck; npm run build;` `/api/extract` Facebook fixture returns normalized facebook `VideoInfo` with MP4 stream; live sample returned 422 no playable streams due auth-wall HTML |
| **US-012** | Facebook Page/Profile video listing | yes | yes | yes | yes | implemented | `cargo fmt --check; cargo check --offline; cargo test --offline` 25 passed; `npm run typecheck; npm run build;` `/api/channel` Facebook fixture follows cursor and returns 3 videos; live bulk ZIP smoke returned 200 application/zip and extracted health payload |

## Evidence Rules

- **Unit proof** covers pure domain and application rules.
- **Integration proof** covers backend enforcement, data integrity, provider behavior, or service contracts.
- **E2E proof** covers user-visible browser flows.
- **Platform proof** covers only shell, deployment, mobile, desktop, or runtime behavior that cannot be proven in lower layers.
- A story can be marked implemented only when its proof requirements are fully validated.
