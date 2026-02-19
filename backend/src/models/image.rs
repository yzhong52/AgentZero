use serde::Serialize;

/// A single image entry associated with a property listing.
/// Can be a local cached file or a remote URL.
#[derive(Serialize, Clone)]
pub struct ImageEntry {
    pub id: i64,
    pub url: String,
    pub created_at: String,
}

/// An image that has been successfully downloaded and cached locally.
/// Only rows with non-null local_path are returned here (used for dedup).
pub struct CachedImage {
    pub sha256: String,
    pub phash: i64,
    pub local_path: String,
}
