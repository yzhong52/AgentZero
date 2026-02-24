use crate::db;
use crate::parsers::ParsedListing;

pub(crate) fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("parsers")
        .join("fixtures")
        .join(name)
}

pub(crate) fn listing_to_property(listing: ParsedListing) -> db::Property {
    let images = listing
        .image_urls
        .into_iter()
        .enumerate()
        .map(|(i, url)| db::ImageEntry {
            id: i as i64,
            url,
            created_at: String::new(),
        })
        .collect();

    db::Property {
        images,
        ..listing.property
    }
}
