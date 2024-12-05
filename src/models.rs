// src/models.rs
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Stock {
    pub symbol: String,
    pub quantity: f64,
    pub buy_price: f64,
}

#[derive(Serialize, Deserialize)]
pub struct Portfolio {
    pub user_id: String,
    pub stocks: Vec<Stock>,
}
