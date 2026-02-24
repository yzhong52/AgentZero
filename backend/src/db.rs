//! Database module - re-exports types and operations from specialized modules.
//! 
//! Organization:
//! - `models`: Data structure definitions (Property, ImageEntry, UserDetails)
//! - `store`: Database operations separated by entity
//!   - `image_store`: Image database operations (read/write)
//!   - `property_store`: Property database operations (read/write)

// Re-export types and operations for backward compatibility
#[allow(unused_imports)]
pub use crate::models::{HistoryEntry, ImageEntry, CachedImage, Property, UserDetails};
pub use crate::store::history_store::{insert_change, list_history};
pub use crate::store::property_store::{init, add_listing, update_by_id, list, get_by_id, update_notes, delete};
pub use crate::store::image_store::{
    list_cached_images, insert_image_url, list_pending_image_urls,
    update_cached_image, list_images_with_meta, get_image_ext,
    delete_image_record, delete_all_image_records,
};

