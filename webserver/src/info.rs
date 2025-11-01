use axum::{Json, http::StatusCode};
use gethostname::gethostname;
use serde::Serialize;
use shared_api::{API_VERSION, MAGIC};

#[derive(Serialize)]
pub struct APIServerInfo {
    magic: String,
    api_version: usize,
    hostname: String,
    version: String,
}

pub async fn server_info() -> Result<Json<APIServerInfo>, StatusCode> {
    let info = APIServerInfo {
        magic: MAGIC.to_string(),
        api_version: API_VERSION,
        hostname: gethostname().into_string().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    Ok(Json(info))
}
