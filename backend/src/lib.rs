pub mod api;
pub mod db;
mod fetch;
mod finance;
pub mod html_snapshots;
pub mod image_paths;
pub mod images;
pub mod ingest;
pub mod models;
pub mod parsers;
pub mod store;
pub mod utils;

pub(crate) use fetch::fetch_html;
pub(crate) use finance::{
	compute_initial_monthly_interest, compute_monthly_cost, compute_monthly_total,
	compute_mortgage,
};
pub(crate) use utils::parse_listing_url;

use axum::{
	routing::{delete, get, patch, post, put},
	Router,
};
use object_store::local::LocalFileSystem;
use reqwest::Client;
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
	let db = db::init(&database_url).await;

	images::ensure_images_dir(IMAGES_LOCAL_DIR).await;
	html_snapshots::ensure_dir().await;
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
			"/api/searches",
			post(api::searches::create_search).get(api::searches::list_searches),
		)
		.route("/api/searches/reorder", put(api::searches::reorder_searches))
		.route(
			"/api/searches/:id",
			get(api::searches::get_search)
				.patch(api::searches::update_search)
				.delete(api::searches::delete_search),
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
		.route("/api/listings/:id/search", patch(api::details::patch_search))
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
