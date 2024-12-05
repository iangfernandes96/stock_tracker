// src/api.rs
use crate::db::{
    add_portfolio, check_data_exists, delete_portfolio, get_portfolio, insert_historical_data,
    query_historical_data, update_portfolio,
};
use crate::error::CustomError;
use crate::models::Portfolio;
use chrono::{DateTime, Duration, Utc};
use log::{error, info};
use reqwest::Client;
use scylla::Session;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

#[derive(Deserialize)]
struct TimeSeriesData {
    #[serde(rename = "1. open")]
    open: String,
    #[serde(rename = "2. high")]
    high: String,
    #[serde(rename = "3. low")]
    low: String,
    #[serde(rename = "4. close")]
    close: String,
    #[serde(rename = "5. volume")]
    volume: String,
}

#[derive(Deserialize)]
struct AlphaVantageResponse {
    #[serde(rename = "Time Series (Daily)")]
    time_series: std::collections::HashMap<String, TimeSeriesData>,
}

pub fn routes(
    session: Arc<Session>,
    api_key: Arc<String>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let add = warp::path("portfolio")
        .and(warp::post())
        .and(with_session(session.clone()))
        .and(warp::body::json())
        .and_then(add_portfolio_handler);

    let get = warp::path!("portfolio" / String)
        .and(warp::get())
        .and(with_session(session.clone()))
        .and_then(get_portfolio_handler);

    let update = warp::path("portfolio")
        .and(warp::put())
        .and(with_session(session.clone()))
        .and(warp::body::json())
        .and_then(update_portfolio_handler);

    let delete = warp::path!("portfolio" / String)
        .and(warp::delete())
        .and(with_session(session.clone()))
        .and_then(delete_portfolio_handler);

    let fetch_and_store = warp::path!("historical" / String)
        .and(warp::get())
        .and(with_session(session.clone()))
        .and(with_api_key(api_key.clone()))
        .and_then(fetch_and_store_handler);

    add.or(get).or(update).or(delete).or(fetch_and_store)
}

fn with_session(
    session: Arc<Session>,
) -> impl Filter<Extract = (Arc<Session>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || session.clone())
}

fn with_api_key(
    api_key: Arc<String>,
) -> impl Filter<Extract = (Arc<String>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || api_key.clone())
}

async fn add_portfolio_handler(
    session: Arc<Session>,
    portfolio: Portfolio,
) -> Result<impl Reply, Rejection> {
    match add_portfolio(&session, portfolio).await {
        Ok(_) => {
            info!("Portfolio added successfully.");
            Ok(warp::reply::with_status(
                "Portfolio added",
                warp::http::StatusCode::CREATED,
            ))
        }
        Err(e) => {
            error!("Failed to add portfolio: {}", e);
            Err(warp::reject::custom(CustomError {
                message: e.to_string(),
            }))
        }
    }
}

async fn get_portfolio_handler(
    user_id: String,
    session: Arc<Session>,
) -> Result<impl Reply, Rejection> {
    match get_portfolio(&session, &user_id).await {
        Ok(portfolio) => {
            info!("Portfolio retrieved successfully.");
            Ok(warp::reply::json(&portfolio))
        }
        Err(e) => {
            error!("Failed to retrieve portfolio: {}", e);
            Err(warp::reject::custom(CustomError {
                message: e.to_string(),
            }))
        }
    }
}

async fn update_portfolio_handler(
    session: Arc<Session>,
    portfolio: Portfolio,
) -> Result<impl Reply, Rejection> {
    match update_portfolio(&session, portfolio).await {
        Ok(_) => {
            info!("Portfolio updated successfully.");
            Ok(warp::reply::with_status(
                "Portfolio updated",
                warp::http::StatusCode::OK,
            ))
        }
        Err(e) => {
            error!("Failed to update portfolio: {}", e);
            Err(warp::reject::custom(CustomError {
                message: e.to_string(),
            }))
        }
    }
}

async fn delete_portfolio_handler(
    user_id: String,
    session: Arc<Session>,
) -> Result<impl Reply, Rejection> {
    match delete_portfolio(&session, &user_id).await {
        Ok(_) => {
            info!("Portfolio deleted successfully.");
            Ok(warp::reply::with_status(
                "Portfolio deleted",
                warp::http::StatusCode::OK,
            ))
        }
        Err(e) => {
            error!("Failed to delete portfolio: {}", e);
            Err(warp::reject::custom(CustomError {
                message: e.to_string(),
            }))
        }
    }
}

async fn fetch_and_store_handler(
    symbol: String,
    session: Arc<Session>,
    api_key: Arc<String>,
) -> Result<impl Reply, Rejection> {
    let end_time = Utc::now();
    let start_time = end_time - Duration::days(30); // Example: last 30 days

    match check_data_exists(&session, &symbol, start_time, end_time).await {
        Ok(true) => {
            info!("Data exists for {}. Fetching from DB.", symbol);
            match query_historical_data(&session, &symbol, start_time, end_time, 50).await {
                Ok(data) => Ok(warp::reply::json(&data)),
                Err(e) => {
                    error!("Failed to query historical data: {}", e);
                    Err(warp::reject::custom(CustomError {
                        message: e.to_string(),
                    }))
                }
            }
        }
        Ok(false) => {
            info!("Data not found for {}. Fetching from provider.", symbol);
            let client = Client::new();
            let url = format!(
                "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY&symbol={}&apikey={}",
                symbol, api_key
            );
            info!("Url {} provider.", url);

            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.json::<AlphaVantageResponse>().await {
                            Ok(api_response) => {
                                let mut fetched_data = Vec::new();
                                for (date_str, data) in api_response.time_series {
                                    if let Ok(date) = DateTime::parse_from_rfc3339(&format!(
                                        "{}T00:00:00Z",
                                        date_str
                                    )) {
                                        let price: f64 = data.close.parse().unwrap_or(0.0);
                                        fetched_data.push((date.with_timezone(&Utc), price));
                                    }
                                }
                                match insert_historical_data(&session, &symbol, fetched_data).await
                                {
                                    Ok(_) => {
                                        let response =
                                            json!({"message": "Data fetched and stored"});
                                        Ok(warp::reply::json(&response))
                                    }
                                    Err(e) => {
                                        error!("Failed to insert historical data: {}", e);
                                        Err(warp::reject::custom(CustomError {
                                            message: e.to_string(),
                                        }))
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Failed to parse API response: {}", e);
                                Err(warp::reject::custom(CustomError {
                                    message: e.to_string(),
                                }))
                            }
                        }
                    } else {
                        error!(
                            "Failed to fetch data from provider: HTTP {}",
                            response.status()
                        );
                        Err(warp::reject::custom(CustomError {
                            message: format!("Failed to fetch data: HTTP {}", response.status()),
                        }))
                    }
                }
                Err(e) => {
                    error!("HTTP request failed: {}", e);
                    Err(warp::reject::custom(CustomError {
                        message: e.to_string(),
                    }))
                }
            }
        }
        Err(e) => {
            error!("Failed to check data existence: {}", e);
            Err(warp::reject::custom(CustomError {
                message: e.to_string(),
            }))
        }
    }
}

pub async fn fetch_and_store_handler_worker(
    symbol: &str,
    session: Arc<Session>,
    client: &Client,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    api_key: Arc<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!(
        "https://www.alphavantage.co/query?function=TIME_SERIES_DAILY&symbol={}&apikey={}",
        symbol, api_key
    );

    let response = client
        .get(&url)
        .send()
        .await?
        .json::<AlphaVantageResponse>()
        .await?;

    for (date, data) in response.time_series {
        let timestamp = date.parse::<DateTime<Utc>>()?;
        if timestamp >= start_time && timestamp <= end_time {
            let price: f64 = data.close.parse()?;
            let timestamp_millis = timestamp.timestamp_millis(); // Convert to milliseconds
            session.query("INSERT INTO stock_tracker.historical_data (symbol, timestamp, price) VALUES (?, ?, ?)", (symbol, timestamp_millis, price)).await?;
            info!("Stored data for {}: {} at {}", symbol, price, timestamp);
        }
    }
    Ok(())
}
