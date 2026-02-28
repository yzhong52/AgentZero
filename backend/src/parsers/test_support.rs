use crate::db;
use crate::models::OpenHouseEvent;
use crate::parsers::ParsedListing;
use serde::Serialize;

pub(crate) fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("parsers")
        .join("fixtures")
        .join(name)
}

/// Snapshot type that captures the full parsed listing: property fields, images,
/// and any open house events extracted by the parser.
#[derive(Serialize)]
pub(crate) struct ListingSnapshot {
    #[serde(flatten)]
    pub property: db::Property,
    pub open_houses: Vec<OpenHouseEvent>,
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

pub(crate) fn listing_to_snapshot(listing: ParsedListing) -> ListingSnapshot {
    let open_houses = listing.open_houses;
    let property = listing_to_property(ParsedListing {
        open_houses: vec![],
        ..listing
    });
    ListingSnapshot { property, open_houses }
}
