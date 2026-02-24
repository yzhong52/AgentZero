use crate::db;
use super::{redfin, rew};

#[test]
fn snapshot_extract_property_829_e14th() {
    let html = std::fs::read_to_string("fixtures/829 E 14th Ave, Vancouver, BC V5T 2N5 _ MLS# R3090427 _ Redfin.html")
        .expect("fixture not found — run from backend/");
    let url = "https://www.redfin.ca/bc/vancouver/829-E-14th-Ave-V5T-2N5/home/155809679";

    let listing = redfin::parse(url, &html).expect("parse failed");
    let images: Vec<db::ImageEntry> = listing.image_urls
        .into_iter()
        .enumerate()
        .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
        .collect();
    let property = db::Property { images, ..listing.property };

    insta::assert_json_snapshot!(property);
}

#[test]
fn snapshot_redfin_788_w8th_unit_l01() {
    let html = std::fs::read_to_string("fixtures/788 W 8th Ave Unit L01, Vancouver, BC V5Z 1E1 _ MLS# R3086230 _ Redfin.html")
        .expect("fixture not found — run from backend/");
    let url = "https://www.redfin.ca/bc/vancouver/788-W-8th-Ave-V5Z-1E1/home/";

    let listing = redfin::parse(url, &html).expect("parse failed");
    let images: Vec<db::ImageEntry> = listing.image_urls
        .into_iter()
        .enumerate()
        .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
        .collect();
    let property = db::Property { images, ..listing.property };

    // HOA must be present for this condo listing
    assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
    insta::assert_json_snapshot!(property);
}

#[test]
fn snapshot_rew_788_w8th_unit_l01() {
    let html = std::fs::read_to_string("fixtures/For Sale_ L01-788 W 8th Avenue, Vancouver, BC - REW.html")
        .expect("fixture not found — run from backend/");
    let url = "https://www.rew.ca/properties/l01-788-w-8th-avenue-vancouver-bc";

    let listing = rew::parse(url, &html).expect("parse failed");
    let images: Vec<db::ImageEntry> = listing.image_urls
        .into_iter()
        .enumerate()
        .map(|(i, img_url)| db::ImageEntry { id: i as i64, url: img_url, created_at: String::new() })
        .collect();
    let property = db::Property { images, ..listing.property };

    // HOA (strata maintenance fee) must be present for this condo listing
    assert_eq!(property.hoa_monthly, Some(1137), "hoa_monthly");
    insta::assert_json_snapshot!(property);
}
