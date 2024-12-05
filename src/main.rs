// src/main.rs
mod api;
mod auth;
mod db;
mod error;
mod models;
mod websocket;

use env_logger::Builder;
use log::{error, info, LevelFilter};
use std::sync::Arc;
use warp::Filter;

#[tokio::main]
async fn main() {
    // Initialize database connection
    Builder::new()
        .filter_level(LevelFilter::Debug)
        .format_timestamp_secs()
        .init();
    let session = match db::init().await {
        Ok(session) => Arc::new(session),
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            return;
        }
    };
    info!("Connected to database...");

    info!("Starting the stock tracker application...");
    let api_key = Arc::new("Y5TT5B05VOFQLQ0R".to_string());

    // Define routes
    let api = api::routes(session, api_key);

    // Start the server
    info!("Server running on http://127.0.0.1:3030");
    warp::serve(api).run(([127, 0, 0, 1], 3030)).await;
}
