// src/main.rs
mod api;
mod auth;
mod db;
mod error;
mod models;
mod websocket;

use crate::api::fetch_and_store_handler_worker;
use chrono::DateTime;
use chrono::Utc;
use env_logger::Builder;
use log::{error, info, LevelFilter};
use reqwest::Client;
use scylla::Session;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time::{self, Duration};
use warp::Filter;

async fn worker(
    session: Arc<Session>,
    client: Client,
    mut rx: mpsc::Receiver<(String, DateTime<Utc>, DateTime<Utc>)>,
    api_key: Arc<String>,
) {
    while let Some((symbol, start_time, end_time)) = rx.recv().await {
        if let Err(e) = fetch_and_store_handler_worker(
            &symbol,
            session.clone(),
            &client,
            start_time,
            end_time,
            api_key.clone(),
        )
        .await
        {
            error!("Error fetching data for {}: {}", symbol, e);
        }
    }
}

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

    let client = Client::new();
    let (tx, rx) = mpsc::channel(100);

    let symbols = vec!["AAPL", "GOOGL", "MSFT", "ABNB", "ADBE"];
    let intervals = vec![
        (Utc::now() - chrono::Duration::days(30), Utc::now()),
        (
            Utc::now() - chrono::Duration::days(60),
            Utc::now() - chrono::Duration::days(30),
        ),
    ];

    for symbol in symbols {
        for (start_time, end_time) in &intervals {
            tx.send((symbol.to_string(), *start_time, *end_time))
                .await
                .unwrap();
        }
    }

    let session_clone = session.clone();
    let client_clone = client.clone();
    let api_key_clone = api_key.clone();
    task::spawn(async move {
        worker(session_clone, client_clone, rx, api_key_clone).await;
    });
    time::sleep(Duration::from_secs(10)).await;

    // Define routes
    let api = api::routes(session, api_key);

    // Start the server
    info!("Server running on http://127.0.0.1:3030");
    warp::serve(api).run(([127, 0, 0, 1], 3030)).await;
}
