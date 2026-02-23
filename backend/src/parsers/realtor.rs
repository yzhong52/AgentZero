//! Realtor.ca listing parser — **not supported; always returns `None`**.
//!
//! # Why Realtor.ca cannot be scraped
//!
//! Realtor.ca uses **Imperva Incapsula** bot protection.  Every direct HTTP
//! request — regardless of headers — is blocked before the listing HTML is
//! served.
//!
//! The block signals are similar to Zillow's PerimeterX:
//!
//! 1. **TLS fingerprint (JA3 hash)** — reqwest/rustls produces a non-browser
//!    TLS handshake that Incapsula identifies immediately.
//!
//! 2. **JavaScript challenge** — Incapsula injects JS that runs in a real
//!    browser and writes a session cookie (`visid_incap_*`).  A plain HTTP
//!    client never executes it.
//!
//! 3. **IP reputation** — IPs without an established browsing history are
//!    auto-blocked.
//!
//! # Alternative source
//!
//! Use **rew.ca** as a supplementary URL for the same property — it is
//! scrapeable and often includes property tax and strata/HOA data that
//! Redfin omits.
//!
//! # Workarounds (not implemented)
//!
//! - **Headless browser** — a real Chrome instance can pass the JS challenge
//!   but is heavy, slow, and against Realtor.ca ToS.
//! - **CREA DDF feed** — official listing data from the Canadian Real Estate
//!   Association; requires registering as a CREA member or data licensee.
//!
//! # Current behaviour
//!
//! `parse()` returns `None`.  The `add_listing` handler in `api/add.rs`
//! detects that all URLs are from blocked hosts and saves a stub so the
//! user can fill in details manually via the edit panel.

use super::ParsedListing;

/// Always returns `None` — see module-level documentation for details.
pub fn parse(_url: &str, _html: &str) -> Option<ParsedListing> {
    None
}
