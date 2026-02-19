//! Models module - contains data structures for the application
//! 
//! Submodules:
//! - `image`: Image and ImageEntry struct definitions
//! - `property`: Property and UserDetails struct definitions

pub mod image;
pub mod property;

pub use image::{ImageEntry, CachedImage};
pub use property::{Property, UserDetails};
