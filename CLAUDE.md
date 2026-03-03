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

### One-command restart scripts
Use these scripts to kill currently running processes, rebuild latest code, and restart fresh instances.

```bash
./scripts/run_backend.sh
./scripts/run_frontend.sh
```

Defaults:
- Backend log: `/tmp/agent_zero_backend.log`
- Frontend log: `/tmp/agent_zero_frontend.log`

## Logs

**Backend log**: `/tmp/agent_zero_backend.log`

Key log patterns to look for when debugging:

| Pattern | Meaning |
|---|---|
| `add_listing: id=N registering 0 image URL(s)` | Parser found no images — likely a sold/off-market listing |
| `redfin::parse: no images in JSON-LD for …` | Redfin sold listing — photos stripped from JSON-LD |
| `Image URL returned error status …` | Image server returned 4xx/5xx — URL probably expired |
| `Failed to download image …` | Network error fetching an image |
| `cache_images: listing_id=N pending=0` | No new images to download (all already cached or none registered) |

## HTML Snapshots

Every time a listing is saved or refreshed, the raw fetched HTML is written to `backend/html_snapshots/` as:

```
{listing_id}_{source}.html   e.g.  42_redfin.html,  42_rew.html
```

Refreshing a listing overwrites the file with the latest fetch. Use these files to:
- Inspect exactly what the parser received
- Re-run or fix the parser locally without re-fetching the URL
- Copy to `backend/src/parsers/fixtures/` (and run the strip tool) to create a test fixture

Optional overrides:
```bash
BACKEND_PORT=8000 LOG_FILE=/tmp/backend.log ./scripts/run_backend.sh
FRONTEND_PORT=5173 LOG_FILE=/tmp/frontend.log ./scripts/run_frontend.sh
```

## API

Route definitions and handler signatures are the source of truth. See:
- `backend/src/main.rs` — all routes wired up
- `backend/src/api/` — one file per handler group (`add.rs`, `refresh.rs`, `listings.rs`, `details.rs`, `images.rs`, `parse.rs`)

## Tests

```bash
cd backend
cargo test
```

Snapshots live in `backend/src/parsers/snapshots/`. To update them after intentional changes:
```bash
cargo insta review
```

### Stripping HTML fixtures

Parser test fixtures live in `backend/src/parsers/fixtures/`. Raw saved pages can be large (700 KB–1.8 MB). Use the strip tool to remove styles, SVGs, tracking scripts, and other bloat while preserving the elements parsers rely on (JSON-LD, `__NEXT_DATA__`, meta tags, property data scripts, DOM elements used by CSS selectors):

```bash
cd backend
cargo run --bin strip                            # strip all fixtures in-place
cargo run --bin strip -- path/to/file.html       # strip a single file in-place
cargo run --bin strip -- input.html output.html  # strip to a separate file
```

Always run `cargo test` after stripping to verify no parser-relevant content was removed.

## Frontend Design

When making layout or styling changes to the frontend, ask if the user would like to load the `frontend-design` skill first (invoke via `/frontend-design`). It provides design thinking guidelines and aesthetic direction to avoid generic UI patterns.

## Coding Conventions

### No code duplication — use shared utilities

Before writing a helper function, check if it already exists elsewhere in the codebase. Shared frontend utilities live in `frontend/src/utils.ts`. Do not copy-paste the same logic into multiple files; extract it to a shared location and import it.

### No hardcoded strings — use named constants

Magic strings that appear in multiple places must be extracted to a named constant and imported from there. This applies to status values, route paths, API keys, and any other string literal that carries semantic meaning.

**Don't:**
```ts
listings.filter(p => p.status === 'Pending')
STATUS_OPTIONS.filter(s => s !== 'Pending')
```

**Do:**
```ts
// constants.ts
export const PENDING_STATUS: StatusOption = 'Pending'

// elsewhere
listings.filter(p => p.status === PENDING_STATUS)
STATUS_OPTIONS.filter(s => s !== PENDING_STATUS)
```

Frontend string constants live in `frontend/src/constants.ts`.

### Rust: prefer named structs over tuples

When a function returns or stores more than one related value, define a small named struct instead of using a tuple. This improves call-site readability and makes fields self-documenting.

**Don't:**
```rust
fn parse_amenity_features(features: &[JsonValue]) -> (Option<i64>, Option<bool>, Option<bool>, Option<bool>) { … }
let (parking, ac, radiant, laundry) = parse_amenity_features(amenities);
```

**Do:**
```rust
struct AmenityFeatures {
    parking_garage: Option<i64>,
    ac: Option<bool>,
    radiant_floor_heating: Option<bool>,
    laundry_in_unit: Option<bool>,
}
fn parse_amenity_features(features: &[JsonValue]) -> AmenityFeatures { … }
let af = parse_amenity_features(amenities);
```

Existing examples in this codebase: `AmenityFeatures`, `AddressInfo`, `SchoolEntry`, `ParsedSource`, `SourceInput` (all in `backend/src/parsers/`).
