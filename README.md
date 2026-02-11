# Property Parser

A small web app that fetches a given URL and parses **property information** (e.g. from real estate or listing pages): address, price, beds, baths, square footage, description, title, and images.

## Run locally

```bash
# From project root
python -m venv .venv
source .venv/bin/activate   # or .venv\Scripts\activate on Windows
pip install -r requirements.txt
uvicorn app.main:app --reload
```

Open [http://127.0.0.1:8000](http://127.0.0.1:8000), paste a listing URL, and click **Parse**.

## API

- **GET /** — Serves the UI.
- **GET /api/parse?url=...** — Fetches the URL and returns JSON with parsed fields:
  - `url`, `title`, `address`, `price`, `beds`, `baths`, `sqft`, `description`, `images`, etc.

Parsing uses JSON-LD (schema.org), meta tags (og/twitter), and common text patterns on the page.
