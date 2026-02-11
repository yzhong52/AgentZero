"""Parse property information from real estate / listing web pages."""

import json
import re
from typing import Any, Optional

from bs4 import BeautifulSoup


def _text(soup: Optional[BeautifulSoup]) -> str:
    if soup is None:
        return ""
    return soup.get_text(strip=True) or ""


def _first_group(text: str, pattern: str) -> str:
    m = re.search(pattern, text, re.I)
    return m.group(1).strip() if m else ""


def extract_json_ld(soup: BeautifulSoup) -> list[dict[str, Any]]:
    """Extract all JSON-LD script blocks (schema.org often used for listings)."""
    scripts = soup.find_all("script", type="application/ld+json")
    result = []
    for s in scripts:
        if not s.string:
            continue
        try:
            data = json.loads(s.string)
            if isinstance(data, list):
                result.extend(data)
            else:
                result.append(data)
        except json.JSONDecodeError:
            continue
    return result


def parse_properties_from_html(html: str, url: str = "") -> dict[str, Any]:
    """
    Parse property-related information from HTML.
    Handles JSON-LD (schema.org), meta tags, and common listing patterns.
    """
    soup = BeautifulSoup(html, "html.parser")
    out: dict[str, Any] = {
        "url": url,
        "address": "",
        "price": "",
        "beds": "",
        "baths": "",
        "sqft": "",
        "description": "",
        "title": "",
        "images": [],
        "raw_json_ld": [],
        "meta": {},
    }

    # --- JSON-LD (RealEstateAgent, Product, Place, etc.) ---
    for ld in extract_json_ld(soup):
        out["raw_json_ld"].append(ld)
        t = ld.get("@type", "")
        if isinstance(t, list):
            t = t[0] if t else ""
        if "Place" in str(t) or "RealEstate" in str(t) or "Product" in str(t):
            addr = ld.get("address")
            if isinstance(addr, dict):
                out["address"] = out["address"] or addr.get("streetAddress", "")
            elif addr and not out["address"]:
                out["address"] = str(addr)
            if isinstance(ld.get("address"), dict):
                addr = ld["address"]
                if not out["address"]:
                    out["address"] = addr.get("streetAddress", "")
                out["meta"]["address_region"] = addr.get("addressRegion", "")
                out["meta"]["address_locality"] = addr.get("addressLocality", "")
            out["price"] = out["price"] or str(ld.get("offers", {}).get("price", "")) if isinstance(ld.get("offers"), dict) else out["price"]
            if isinstance(ld.get("offers"), dict) and not out["price"]:
                out["price"] = str(ld["offers"].get("price", ""))
            out["description"] = out["description"] or _text(BeautifulSoup(ld.get("description", ""), "html.parser"))
            out["title"] = out["title"] or ld.get("name", "")
        if "Product" in str(t):
            offers = ld.get("offers")
            if isinstance(offers, dict) and not out["price"]:
                out["price"] = str(offers.get("price", ""))
            elif isinstance(offers, list) and offers and not out["price"]:
                out["price"] = str(offers[0].get("price", ""))

    # --- Meta tags (og:, twitter:, description) ---
    for meta in soup.find_all("meta", property=re.compile(r"^(og:|twitter:)")):
        prop = meta.get("property") or meta.get("name") or ""
        content = meta.get("content", "")
        if "title" in prop.lower():
            out["title"] = out["title"] or content
        elif "description" in prop.lower():
            out["description"] = out["description"] or content
        elif "image" in prop.lower() and content and content not in out["images"]:
            out["images"].append(content)
    desc_meta = soup.find("meta", attrs={"name": "description"})
    if desc_meta and not out["description"]:
        out["description"] = desc_meta.get("content", "")

    # --- Common listing patterns in text ---
    body_text = _text(soup.find("body"))
    if not out["price"]:
        out["price"] = _first_group(body_text, r"(\$[\d,]+(?:\.\d{2})?)")
    if not out["beds"]:
        out["beds"] = _first_group(body_text, r"(\d+)\s*(?:bed|bd|br)\b")
    if not out["baths"]:
        out["baths"] = _first_group(body_text, r"(\d+(?:\.\d+)?)\s*(?:bath|ba)\b")
    if not out["sqft"]:
        m = re.search(r"([\d,]+)\s*sq\.?\s*ft\.?|([\d,]+)\s*sf\b", body_text, re.I)
        out["sqft"] = (m.group(1) or m.group(2) or "").strip() if m else ""
    if not out["sqft"]:
        out["sqft"] = _first_group(body_text, r"(\d[\d,]*)\s*sqft")

    # --- Title from page ---
    if not out["title"]:
        t = soup.find("title")
        out["title"] = _text(t) if t else ""

    # --- Images from common listing markup ---
    for img in soup.find_all("img", src=True)[:20]:
        src = img.get("src", "")
        if src and ("photo" in src.lower() or "image" in src.lower() or "listing" in src.lower() or "property" in src.lower()):
            if src not in out["images"]:
                out["images"].append(src)
    if not out["images"]:
        for img in soup.find_all("img", src=True)[:5]:
            src = img.get("src", "")
            if src and not any(x in src for x in ["logo", "icon", "pixel", "track"]):
                out["images"].append(src)

    # Clean empty lists for nicer JSON
    if not out["raw_json_ld"]:
        del out["raw_json_ld"]
    if not out["meta"]:
        del out["meta"]
    return out
