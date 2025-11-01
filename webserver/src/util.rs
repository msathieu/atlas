use axum::http::StatusCode;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn convert_timestamp(time: SystemTime) -> Result<u64, StatusCode> {
    Ok(time.duration_since(UNIX_EPOCH).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.as_secs())
}
