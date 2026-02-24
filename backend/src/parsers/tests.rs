use crate::db;
use super::{redfin, rew};

/// Returns the path to a fixture file, anchored to the crate root via
/// `CARGO_MANIFEST_DIR` so the tests work regardless of working directory.
macro_rules! fixture {
    ($name:expr) => {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join($name)
    };
}

/// Build a `Property` from a `ParsedListing`, attaching image URLs as stubs.
fn listing_to_property(listing: super::ParsedListing) -> db::Property {
    let images = listing.image_urls
        .into_iter()
        .enumerate()
        .map(|(i, url)| db::ImageEntry { id: i as i64, url, created_at: String::new() })
        .collect();
    db::Property { images, ..listing.property }
}

// ── Redfin ───────────────────────────────────────────────────────────────────

#[test]
fn redfin_829_e14th() {
    let html = std::fs::read_to_string(fixture!("redfin_829_e14th.html"))
        .expect("fixture not found");
    let listing = redfin::parse(
        "https://www.redfin.ca/bc/vancouver/829-E-14th-Ave-V5T-2N5/home/155809679",
        &html,
    ).expect("parse failed");
    insta::assert_json_snapshot!("redfin_829_e14th", listing_to_property(listing));
}

#[test]
fn redfin_788_w8th() {
    let html = std::fs::read_to_string(fixture!("redfin_788_w8th.html"))
        .expect("fixture not found");
    let listing = redfin::parse(
        "https://www.redfin.ca/bc/vancouver/788-W-8th-Ave-V5Z-1E1/home/",
        &html,
    ).expect("parse failed");
    let property = listing_to_property(listing);
    assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
    insta::assert_json_snapshot!("redfin_788_w8th", property);
}

// ── REW ──────────────────────────────────────────────────────────────────────

#[test]
fn rew_788_w8th() {
    let html = std::fs::read_to_string(fixture!("rew_788_w8th.html"))
        .expect("fixture not found");
    let listing = rew::parse(
        "https://www.rew.ca/properties/l01-788-w-8th-avenue-vancouver-bc",
        &html,
    ).expect("parse failed");
    let property = listing_to_property(listing);
    assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
    insta::assert_json_snapshot!("rew_788_w8th", property);
}
