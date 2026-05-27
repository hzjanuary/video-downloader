# Frontend

Next.js App Router dashboard UI that provides a rich, responsive interface for fetching collections, managing video selection, and coordinating bulk downloads.

## Features

- **Platform Input**: Accepts YouTube channel/playlist, TikTok profile, and Facebook page/profile URLs.
- **Stateless Authentication Bypass**: Optional provider cookie text fields let users submit active session strings for auth-walled scenarios.
- **Dynamic Selection Grid**: Fetches collection feeds and renders video cards with titles and thumbnails, letting users multi-select items or use `Chọn tất cả` (Select All).
- **On-the-fly Archiver**: Communicates with the backend streaming endpoint to retrieve multiple media files packed as a single ZIP file download directly in the browser.
- **Health Indicator**: Automatically fetches the backend API health status on mount and displays connection feedback.

## Configuration

The frontend determines the backend API target using the `NEXT_PUBLIC_API_BASE_URL` environment variable:

- **Development Default**: `http://localhost:8080`
- **Production URL**: Can be configured by writing to a `.env.local` file or exporting the environment variable directly.

---

## Commands

### Install Dependencies
```bash
npm install
```

### Start Development Server
```bash
npm run dev
```
Serves the dynamic interface locally at `http://localhost:3000`.

### Compile & Build Production Bundle
```bash
npm run build
```

### Typecheck Source Code
```bash
npm run typecheck
```
