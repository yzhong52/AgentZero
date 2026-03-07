# AgentZero

AgentZero is a property listing parser used to fetch and persist real estate
listing data from multiple sources (Redfin, Realtor, REW, Zillow, etc.).
It combines a Rust/Axum backend with a React/TypeScript frontend and is
optimized for debugging, snapshot testing, and developer productivity.

**Backend:** Rust (Axum)  
**Frontend:** React + TypeScript (Vite)

---

## 🚀 Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+) and npm

### 🛠️ Build & Run

```bash
# install frontend deps
npm install

# build frontend bundle
npm run build

# start backend server (from project root)
cargo run --release
# it binds to http://127.0.0.1:8000 by default
```

For development with live reload:

```bash
# terminal 1 – watch & rebuild frontend
npm run dev

# terminal 2 – run backend with cargo-watch
cargo install cargo-watch   # one‑time
cargo watch -x run
```

### ⚡ One‑command restart scripts

There are convenience helpers that kill any running processes, rebuild, and
restart both services:

```bash
./scripts/run_backend.sh
./scripts/run_frontend.sh
```

You can override ports or log files using environment variables:

```bash
BACKEND_PORT=8001 LOG_FILE=/tmp/backend.log ./scripts/run_backend.sh
```

---

## 📁 Project Layout

```
README.md                         ← you are here
backend/                          ← Rust server
  Cargo.toml
  src/
    api/                          ← HTTP handlers
    parsers/                      ← parsing logic + fixtures/snapshots
    models/ store/ images/ etc.
frontend/                         ← React/Vite application
  src/
  bindings/                       ← auto‑generated TS from Rust models
scripts/                          ← helper restart scripts
```

---

## 🧠 API Overview

The HTTP routes are defined in `backend/src/main.rs` and grouped by handler
files in `backend/src/api/`.

Key endpoints:

| Method | Path                                  | Description                        |
|--------|---------------------------------------|------------------------------------|
| GET    | `/`                                   | Serve the UI                       |
| GET    | `/api/listings/:id/preview`           | Parse but don’t save (refresh diff)|
| PUT    | `/api/listings/:id/refresh`           | Re‑fetch, parse, merge, and save   |
| POST   | `/api/parse?url=…`                    | Fetch arbitrary URL and parse

(The frontend mainly uses the listing endpoints.)

Source of truth for parsing logic lives under `backend/src/parsers/` and
unit tests use HTML fixtures stored in `backend/src/parsers/fixtures/`.

---

## 🧪 Testing

Backend tests are standard Cargo tests:

```bash
cd backend
git fetch && cargo test
```

Parser fixtures are snapshot‑tested with [`cargo-insta`]. After fixing
parsing behavior you’ll update snapshots with:

```bash
cargo insta review
```

To strip bulky HTML fixtures and keep only the elements the parsers care
about, use the helper binary:

```bash
cd backend
cargo run --bin strip          # strip all fixtures in place
cargo run --bin strip -- path/to/file.html
```

Always run the full test suite after stripping to ensure no parser data was
removed.

---

## 🔗 TypeScript Type Bindings

Rust models are annotated with `#[cfg_attr(test, derive(TS))]` (via
[`ts-rs`]). Running `cargo test` regenerates the reference files under
`frontend/src/bindings/`. They are committed as a drift‑detection signal; do
not import them directly — use `frontend/src/types.ts` instead.

Workflow when changing a backend model:

1. Modify the struct in `backend/src/models/`.
2. Run `cargo test`; the binding file in `bindings/` will update.
3. Update `frontend/src/types.ts` accordingly.

---

## 🧩 Frontend Design & Conventions

When making UI/UX changes, you can invoke the `frontend-design` skill in the
agent (`/frontend-design`) for guidance on layout and aesthetics. It helps
avoid generic patterns and keep the app visually coherent.

### Coding Standards

- **No duplication**: search `frontend/src/utils.ts` before adding helpers.
- **Constants**: extract commonly used strings to named constants in
  `frontend/src/constants.ts` rather than hard‑coding them.

---

## 🛠️ Backend Conventions

- Prefer named structs over tuples for multi‑value returns; examples already
  exist in `backend/src/parsers/` (e.g. `AmenityFeatures`, `AddressInfo`).
- The merge logic in `backend/src/api/refresh.rs` is intentionally exhaustive
  to force conscious decisions when adding new fields.

See the file headers and comments throughout the repo for additional
documentation.

---

## 📦 Contributing

Feel free to open issues or pull requests. Ensure new features include
appropriate tests and update documentation where necessary.

---

[ts-rs]: https://github.com/Aleph-Alpha/ts-rs



