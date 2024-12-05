// src/auth.rs
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

// Function to create a JWT token
pub fn create_token(user_id: &str) -> String {
    let claims = Claims {
        sub: user_id.to_string(),
        exp: 10000000000, // Set expiration
    };
    let secret_key = "your_secret_key";
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret_key.as_ref()),
    )
    .unwrap()
}
