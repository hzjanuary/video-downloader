# Platform Contract

## Runtime Surfaces

- Frontend: Next.js App Router in `frontend`, served on port `3000`.
- Backend: Rust Axum API in `backend`, served on port `8080`.

## Health Integration

- Backend exposes `GET /api/health`.
- The route returns the text `OK: backend is healthy`.
- Backend CORS allows the frontend development origin
  `http://localhost:3000`.
- Frontend fetches the backend health endpoint through
  `NEXT_PUBLIC_API_BASE_URL`, defaulting to `http://localhost:8080`, and
  renders the returned text on the first page.

## Provider Cookie Input

- The frontend exposes an optional provider cookie field.
- The backend accepts the cookie on `GET /api/extract`, `GET /api/channel`, and
  `POST /api/download/bulk`.
- The cookie is forwarded only to upstream provider fetch/download requests and
  is not stored by the application.
