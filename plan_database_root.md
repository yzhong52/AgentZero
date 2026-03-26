# Plan: Configurable Database Root Folder via `AGENT_ZERO_DATA_DIR`

## Overview

Introduce a single environment variable `AGENT_ZERO_DATA_DIR` that controls where
all persistent data (SQLite database, cached images, HTML snapshots) lives.

| What | Current (hardcoded, relative to `backend/`) | New default | Your override |
|---|---|---|---|
| SQLite DB | `backend/listings.db` | `~/.agent_zero/listings.db` | `~/Projects/AgentZero/database/listings.db` |
| Image cache | `backend/listings_images/` | `~/.agent_zero/listings_images/` | `~/Projects/AgentZero/database/listings_images/` |
| HTML snapshots | `backend/html_snapshots/` | `~/.agent_zero/html_snapshots/` | `~/Projects/AgentZero/database/html_snapshots/` |

---

## Step-by-step Plan

### 1. Write this plan file ✅

### 2. Update `backend/src/lib.rs`

- Read `AGENT_ZERO_DATA_DIR` at startup (expand `~` via `dirs::home_dir()`).
- Default to `~/.agent_zero`.
- Derive paths:
  - `db_path   = {data_dir}/listings.db`
  - `img_dir   = {data_dir}/listings_images`
  - `snap_dir  = {data_dir}/html_snapshots`
- Replace the hardcoded `IMAGES_LOCAL_DIR` constant with a runtime value.
- Pass `snap_dir` into `html_snapshots::ensure_dir()` and make it available to snapshot writers.

### 3. Update `backend/src/fetching/html_snapshots.rs`

- Remove the hardcoded `DIR` constant.
- Accept the snapshots directory at startup via a `once_cell::sync::OnceCell<PathBuf>` (or thread-local / function argument).
- `ensure_dir(dir)` takes the path as an argument.
- `save_listing_html(listing_id, site, html)` reads from the cell.

### 4. Update `backend/src/images/paths.rs`

- Replace the static `IMAGES_LOCAL_DIR` import with a runtime-resolved path.
- Use the same `OnceCell` pattern or pass the path through `AppState`.

### 5. Add `dirs` crate dependency

- Add `dirs = "5"` to `backend/Cargo.toml` (for `~` expansion).

### 6. Move existing data files

```bash
mkdir -p ~/Projects/AgentZero/database
mv backend/listings.db         ~/Projects/AgentZero/database/
mv backend/listings_images     ~/Projects/AgentZero/database/
mv backend/html_snapshots      ~/Projects/AgentZero/database/
```

### 7. Update `~/.zshrc`

Add:
```zsh
export AGENT_ZERO_DATA_DIR="$HOME/Projects/AgentZero/database"
```

### 8. Verify

```bash
source ~/.zshrc
cd backend && cargo build --release
# Confirm it starts and reads from the new location
```

---

## Why a single `AGENT_ZERO_DATA_DIR` (not separate vars)?

All three artefacts are co-located and managed together. One var is simpler to
set/change and avoids drift between them. `DATABASE_URL` is kept but only as an
explicit override if someone needs to point the DB somewhere else entirely.
