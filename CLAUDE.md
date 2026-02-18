# AgentZero

Property listing parser — Rust (Axum) backend + React/TypeScript (Vite) frontend.

## Running locally

### Backend
```bash
cd backend
cargo run --release
```
Starts at `http://127.0.0.1:8000`. Creates `backend/listings.db` on first run.

### Frontend (dev server)
```bash
npm --prefix frontend run dev
```
Starts at `http://localhost:5173/`. Proxies `/api/*` to the backend.

## API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/parse?url=<url>` | Fetch & parse a listing URL (no DB write) |
| POST | `/api/listings` | `{"url":"..."}` — parse + save to DB |
| GET | `/api/listings` | Return all saved listings (newest first) |

## Tests

```bash
cd backend
cargo test
```

Snapshots live in `backend/src/snapshots/`. To update them after intentional changes:
```bash
cargo insta review
```
