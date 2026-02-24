pub mod db;
pub mod image_paths;
pub mod images;
pub mod models;
pub mod store;

/// URL prefix under which cached images are served (must match the `nest_service` mount point).
/// Used by `image_paths::serve_url` to construct the serve URL, e.g. `/images/1/abc.jpg`.
pub const IMAGES_URL_PREFIX: &str = "/images";

/// Filesystem directory where downloaded images are stored.
/// Used both to initialise the object store and to clean up per-listing subdirectories on delete.
pub const IMAGES_LOCAL_DIR: &str = "listings_images";
