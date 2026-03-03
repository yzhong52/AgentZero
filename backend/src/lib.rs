pub mod api;
mod finance;
pub mod fetching;
pub mod images;
pub mod models;
pub mod parsers;
pub mod store;

use axum::{
	routing::{delete, get, patch, post, put},
	Router,
};
use object_store::local::LocalFileSystem;
use reqwest::Client;
use crate::store::property_store;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

/// URL prefix under which cached images are served (must match the `nest_service` mount point).
/// Used by `image_paths::serve_url` to construct the serve URL, e.g. `/images/1/abc.jpg`.
pub const IMAGES_URL_PREFIX: &str = "/images";

/// Filesystem directory where downloaded images are stored.
/// Used both to initialise the object store and to clean up per-listing subdirectories on delete.
pub const IMAGES_LOCAL_DIR: &str = "listings_images";

#[derive(Clone)]
pub(crate) struct AppState {
	pub(crate) db: sqlx::SqlitePool,
	pub(crate) client: Client,
	pub(crate) store: Arc<dyn object_store::ObjectStore>,
}

pub async fn build_app() -> Router {
	let database_url =
		std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://listings.db".to_string());
	let db = property_store::init(&database_url).await;

	images::ensure_images_dir(IMAGES_LOCAL_DIR).await;
	fetching::html_snapshots::ensure_dir().await;
	let store: Arc<dyn object_store::ObjectStore> = Arc::new(
		LocalFileSystem::new_with_prefix(std::path::Path::new(IMAGES_LOCAL_DIR))
			.expect("Failed to initialize local image store"),
	);

	let client = Client::builder()
		.timeout(Duration::from_secs(15))
		.build()
		.unwrap();

	let state = AppState { db, client, store };

	let cors = CorsLayer::new()
		.allow_origin(
			"http://localhost:5173"
				.parse::<axum::http::HeaderValue>()
				.unwrap(),
		)
		.allow_methods(Any)
		.allow_headers(Any);

	Router::new()
		.route("/api/parse", get(api::parse::parse))
		.route(
			"/api/search-profiles",
			post(api::search_profiles::create_search_profile)
				.get(api::search_profiles::list_search_profiles),
		)
		.route(
			"/api/search-profiles/reorder",
			put(api::search_profiles::reorder_search_profiles),
		)
		.route(
			"/api/search-profiles/:id",
			get(api::search_profiles::get_search_profile)
				.patch(api::search_profiles::update_search_profile)
				.delete(api::search_profiles::delete_search_profile),
		)
		.route(
			"/api/listings",
			post(api::add::add_listing).get(api::listings::list_listings),
		)
		.route("/api/listings/:id", get(api::listings::get_listing))
		.route("/api/listings/:id/delete", delete(api::listings::delete_listing))
		.route("/api/listings/:id/refresh", put(api::refresh::refresh_listing))
		.route("/api/listings/:id/preview", get(api::refresh::preview_refresh))
		.route("/api/listings/:id/notes", patch(api::details::patch_notes))
		.route("/api/listings/:id/search-profile", patch(api::details::patch_search_profile))
		.route("/api/listings/:id/details", patch(api::details::patch_details))
		.route("/api/listings/:id/history", get(api::details::get_history))
		.route(
			"/api/listings/:id/open-houses",
			get(api::open_houses::get_open_houses),
		)
		.route(
			"/api/listings/:id/open-houses/:oh_id",
			patch(api::open_houses::patch_open_house),
		)
		.route(
			"/api/listings/:id/images/:image_id",
			delete(api::images::delete_image),
		)
		.nest_service(IMAGES_URL_PREFIX, ServeDir::new(IMAGES_LOCAL_DIR))
		.with_state(state)
		.layer(cors)
}

pub async fn run() {
	tracing_subscriber::fmt::init();
	let app = build_app().await;
	let bind = "127.0.0.1:8000";
	println!("Starting backend at http://{}", bind);
	let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
	axum::serve(listener, app).await.unwrap();
}
