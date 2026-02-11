"""FastAPI app: parse property info from a given URL."""

from pathlib import Path
from urllib.parse import urlparse

import httpx
from fastapi import FastAPI, HTTPException
from fastapi.responses import HTMLResponse
from fastapi.staticfiles import StaticFiles

from app.parser import parse_properties_from_html

app = FastAPI(title="Property Parser", description="Parse property information from a URL")

BASE = Path(__file__).resolve().parent.parent
app.mount("/static", StaticFiles(directory=BASE / "static"), name="static")


def _is_safe_url(url: str) -> bool:
    try:
        parsed = urlparse(url)
        if not parsed.scheme in ("http", "https"):
            return False
        if not parsed.netloc or "." not in parsed.netloc:
            return False
        return True
    except Exception:
        return False


@app.get("/", response_class=HTMLResponse)
async def index():
    """Serve the single-page form."""
    with open(BASE / "static" / "index.html") as f:
        return f.read()


@app.get("/api/parse")
async def parse_url(url: str):
    """
    Fetch the given URL and return parsed property information.
    """
    if not url or not url.strip():
        raise HTTPException(status_code=400, detail="Missing 'url' query parameter")
    url = url.strip()
    if not _is_safe_url(url):
        raise HTTPException(status_code=400, detail="Invalid or disallowed URL")

    headers = {
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
    }
    async with httpx.AsyncClient(follow_redirects=True, timeout=15.0, headers=headers) as client:
        try:
            resp = await client.get(url)
            if resp.status_code == 429:
                raise HTTPException(
                    status_code=429,
                    detail="This site is rate-limiting requests. Try again in a few minutes, or paste the page HTML if you have it.",
                )
            resp.raise_for_status()
            html = resp.text
        except HTTPException:
            raise
        except httpx.HTTPError as e:
            raise HTTPException(status_code=502, detail=f"Failed to fetch URL: {e!s}")

    data = parse_properties_from_html(html, url=url)
    return data
