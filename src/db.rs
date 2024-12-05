// src/db.rs
use crate::models::{Portfolio, Stock};
use chrono::{DateTime, NaiveDateTime, Utc};
use log::error;
use log::info;
use scylla::{frame::response::result::CqlValue, query::Query, Session, SessionBuilder};

pub async fn init() -> Result<Session, Box<dyn std::error::Error>> {
    let session = SessionBuilder::new()
        .known_node("127.0.0.1:9042")
        .build()
        .await
        .expect("Failed to connect to ScyllaDB");

    // Create keyspace and tables if they don't exist
    // ... (Add your schema setup here)
    // Create keyspace and tables if they don't exist
    session.query("CREATE KEYSPACE IF NOT EXISTS stock_tracker WITH REPLICATION = {'class': 'SimpleStrategy', 'replication_factor': 1}", &[]).await?;
    session.query("CREATE TABLE IF NOT EXISTS stock_tracker.portfolios (user_id TEXT PRIMARY KEY, stocks TEXT)", &[]).await?;
    session.query("CREATE TABLE IF NOT EXISTS stock_tracker.historical_data (symbol TEXT, timestamp TIMESTAMP, price DOUBLE, PRIMARY KEY (symbol, timestamp)) WITH CLUSTERING ORDER BY (timestamp DESC)", &[]).await?;

    info!("Successfully connected to ScyllaDB.");
    Ok(session)
}

pub async fn add_portfolio(
    session: &Session,
    portfolio: Portfolio,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stocks_json = serde_json::to_string(&portfolio.stocks)?;
    let query = Query::new("INSERT INTO stock_tracker.portfolios (user_id, stocks) VALUES (?, ?)");
    session
        .query(query, (portfolio.user_id, stocks_json))
        .await?;
    Ok(())
}

pub async fn get_portfolio(
    session: &Session,
    user_id: &str,
) -> Result<Portfolio, Box<dyn std::error::Error + Send + Sync>> {
    let query = Query::new("SELECT stocks FROM stock_tracker.portfolios WHERE user_id = ?");
    if let Some(row) = session
        .query(query, (user_id,))
        .await?
        .rows
        .unwrap()
        .into_iter()
        .next()
    {
        let stocks_json: String = row.columns[0]
            .as_ref()
            .unwrap()
            .as_text()
            .unwrap()
            .to_string();
        let stocks: Vec<Stock> = serde_json::from_str(&stocks_json)?;
        Ok(Portfolio {
            user_id: user_id.to_string(),
            stocks,
        })
    } else {
        Err("Portfolio not found".into())
    }
}

pub async fn update_portfolio(
    session: &Session,
    portfolio: Portfolio,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    add_portfolio(session, portfolio).await
}

pub async fn delete_portfolio(
    session: &Session,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let query = Query::new("DELETE FROM stock_tracker.portfolios WHERE user_id = ?");
    session.query(query, (user_id,)).await?;
    Ok(())
}

pub async fn check_data_exists(
    session: &Session,
    symbol: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let start_timestamp = start_time.timestamp_millis();
    let end_timestamp = end_time.timestamp_millis();

    let query = Query::new("SELECT COUNT(*) FROM stock_tracker.historical_data WHERE symbol = ? AND timestamp >= ? AND timestamp <= ?");
    let result = session
        .query(query, (symbol, start_timestamp, end_timestamp))
        .await?;
    let count: i64 = result.rows.unwrap().into_iter().next().unwrap().columns[0]
        .as_ref()
        .unwrap()
        .as_bigint()
        .unwrap();
    Ok(count > 0)
}

pub async fn insert_historical_data(
    session: &Session,
    symbol: &str,
    data: Vec<(DateTime<Utc>, f64)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let query = Query::new(
        "INSERT INTO stock_tracker.historical_data (symbol, timestamp, price) VALUES (?, ?, ?)",
    );
    for (timestamp, price) in data {
        let timestamp_millis = timestamp.timestamp_millis();
        session
            .query(query.clone(), (symbol, timestamp_millis, price))
            .await?;
    }
    Ok(())
}

pub async fn query_historical_data(
    session: &Session,
    symbol: &str,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    limit: i32,
) -> Result<Vec<(DateTime<Utc>, f64)>, Box<dyn std::error::Error + Send + Sync>> {
    let start_timestamp = start_time.timestamp_millis();
    let end_timestamp = end_time.timestamp_millis();

    let query = Query::new("SELECT timestamp, price FROM stock_tracker.historical_data WHERE symbol = ? AND timestamp >= ? AND timestamp <= ? LIMIT ?");
    let result = session
        .query(query, (symbol, start_timestamp, end_timestamp, limit))
        .await?;

    if let Some(rows) = result.rows {
        let data: Vec<(DateTime<Utc>, f64)> = rows
            .into_iter()
            .filter_map(|row| {
                let timestamp_millis: Option<_> = row.columns[0].as_ref().and_then(|v| match v {
                    CqlValue::Timestamp(ts) => Some(*ts),
                    _ => None,
                });
                let price: Option<f64> = row.columns[1].as_ref().and_then(|col| col.as_double());
                match (timestamp_millis, price) {
                    (Some(ts), Some(p)) => {
                        let dt = DateTime::<Utc>::from_timestamp_millis(ts.num_milliseconds())
                            .unwrap_or_default();
                        Some((dt, p))
                    }

                    _ => {
                        error!(
                            "Missing data in row: timestamp = {:?}, price = {:?}",
                            timestamp_millis, price
                        );
                        None
                    }
                }
            })
            .collect();
        info!("Fetched {} records for symbol: {}", data.len(), symbol);
        Ok(data)
    } else {
        Err("No rows found".into())
    }
}
