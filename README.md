# Property Parser

A small web app that fetches a given URL and parses **property information** (e.g. from real estate or listing pages): address, price, beds, baths, square footage, description, title, and images.

**Backend:** Rust (Axum)  
**Frontend:** TypeScript

## Setup

### Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+) and npm

### Build & Run

```bash
# Install frontend dependencies
npm install

# Build TypeScript frontend
npm run build

# Run Rust backend (from project root)
cargo run --bin property_parser --release 2>&1 | tail -20
```

The server will start on [http://127.0.0.1:8001](http://127.0.0.1:8001)

For development with auto-reload:

```bash
# Terminal 1: Watch and rebuild TypeScript
npm run dev

# Terminal 2: Run Rust server (auto-reloads on code changes with cargo-watch)
cargo install cargo-watch  # one-time install
cargo watch -x run
```

## API

- **GET /** — Serves the UI.
- **GET /api/parse?url=...** — Fetches the URL and returns JSON with parsed fields:
  - `url`, `title`, `address`, `price`, `beds`, `baths`, `sqft`, `description`, `images`, `meta`, etc.

Parsing uses JSON-LD (schema.org), meta tags (og/twitter), and common text patterns on the page.

## Project Structure

```
├── src/
│   ├── main.rs      # Rust web server (Axum)
│   ├── parser.rs    # Property parsing logic
│   └── app.ts       # TypeScript frontend code
├── static/
│   └── index.html   # HTML template
├── Cargo.toml       # Rust dependencies
├── package.json     # Node/TypeScript dependencies
└── tsconfig.json    # TypeScript config
```
