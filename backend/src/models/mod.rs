//! Models module - contains data structures for the application
//!
//! Submodules:
//! - `image`: Image and ImageEntry struct definitions
//! - `property`: Property and UserDetails struct definitions

pub mod history;
pub mod image;
pub mod property;
pub mod search;

pub use history::HistoryEntry;
pub use image::{CachedImage, ImageEntry};
pub use property::{Property, UserDetails};
pub use search::Search;
