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

Optional overrides:
```bash
BACKEND_PORT=8000 LOG_FILE=/tmp/backend.log ./scripts/run_backend.sh
FRONTEND_PORT=5173 LOG_FILE=/tmp/frontend.log ./scripts/run_frontend.sh
```

## API

Below are the public HTTP endpoints exposed by the backend. All endpoints return JSON unless noted.

- **GET /api/parse?url=<url>**
	- Description: Fetch the given URL and run the parser, returning parsed fields (title, description, images, address, price, beds, baths, sqft, etc.). This does not write to the DB.
	- Response (200): { "url": "...", "title": "...", "description": "...", "images": ["..."], "meta": {...} }

- **POST /api/listings**
	- Description: Parse and save one or more listing source URLs as a single property. Use when you have multiple source URLs (Redfin, Realtor, etc.) for the same property.
	- Body (JSON): { "urls": ["https://redfin.example/...", "https://rew.example/..."] }
	- Response (200): the saved `Property` record (includes `id`, parsed fields, and `images` metadata).
	- Errors: 400 for invalid request, 502 for fetch failures, 422 if no supported listing format found.

- **GET /api/listings**
	- Description: Return all saved properties, newest first. Each record includes cached `images` metadata (id, local_path, position).

- **GET /api/listings/:id**
	- Description: Get a single property by ID (includes images and metadata).

- **PUT /api/listings/:id**
	- Description: Refresh an existing listing by re-fetching its stored source URLs and re-parsing; overwrites parsed fields but preserves user-edits like mortgage settings.

- **DELETE /api/listings/:id**
	- Description: Delete a property and associated data (images cascade via FK).

- **GET /api/listings/:id/preview**
	- Description: Run a preview refresh (fetch/parse) without saving changes — useful for validating parser output.

- **PATCH /api/listings/:id/notes**
	- Description: Update the `notes` field. Body: `{ "notes": "..." }`.

- **PATCH /api/listings/:id/nickname**
	- Description: Update the user-visible `nickname`/alias. Body: `{ "nickname": "My shortlist" }`.

- **PATCH /api/listings/:id/details**
	- Description: Apply user-edited fields (partial) to a listing. Accepts the same shape as `UserDetails` in the codebase — common fields: `price`, `price_currency`, `offer_price`, `street_address`, `city`, `bedrooms`, `bathrooms`, `sqft`, `year_built`, `mortgage_monthly`, etc.
	- Body (example): `{ "price": 110000, "city": "Vancouver", "bedrooms": 3 }`

- **GET /api/listings/:id/history**
	- Description: Get the change history for a property (price changes etc.).

- **DELETE /api/listings/:id/images/:image_id**
	- Description: Delete a cached image for the listing. This removes the DB record and the underlying file/object-store key.

Notes:
- Cached images are served from `/images/<object_key>` by the webserver (local filesystem in dev). The stored `local_path` values are of the form `/images/<listing_id>/<sha256>.<ext>`.
- Most endpoints return `500` for unexpected DB errors; handlers attempt to translate fetch/parse failures into `502` / `422` where appropriate.

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
