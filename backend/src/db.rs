//! Database module - re-exports types and operations from specialized modules.
//!
//! Organization:
//! - `models`: Data structure definitions (Property, ImageEntry, UserDetails)
//! - `store`: Database operations separated by entity
//!   - `image_store`: Image database operations (read/write)
//!   - `property_store`: Property database operations (read/write)

// Re-export types and operations for backward compatibility
#[allow(unused_imports)]
pub use crate::models::{CachedImage, HistoryEntry, ImageEntry, Property, Search, UserDetails};
pub use crate::store::history_store::{insert_change, list_history};
pub use crate::store::image_store::{
    delete_all_image_records, delete_image_record, get_image_ext, insert_image_url,
    list_cached_images, list_images_with_meta, list_pending_image_urls, update_cached_image,
};
pub use crate::store::property_store::{
    add_listing, delete, find_by_mls, find_by_source_url, get_by_id, init, list, update_by_id,
    update_notes, update_search_id,
};
pub use crate::store::search_store;
