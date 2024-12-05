// src/error.rs
use std::fmt;
use warp::reject::Reject;

#[derive(Debug)]
pub struct CustomError {
    pub message: String,
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CustomError {}

impl Reject for CustomError {}
