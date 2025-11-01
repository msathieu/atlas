use crate::{
    auth::{authenticate, authenticate_jwt, generate_jwt},
    get_db,
};
use axum::{
    Json,
    extract::Query,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use shared_server::binary::personalize_binary;

#[derive(Deserialize)]
pub struct DownloadAuth {
    pub auth: String,
}
#[derive(Serialize)]
pub struct DownloadAuthResponse {
    jwt: String,
}

pub async fn download_agent(query: Query<DownloadAuth>) -> Result<Vec<u8>, StatusCode> {
    authenticate_jwt(&query.auth, "webserver-download").await?;
    let mut db = get_db()?;
    let binary = personalize_binary(&mut db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(binary)
}
pub async fn download_auth(headers: HeaderMap) -> Result<Json<DownloadAuthResponse>, StatusCode> {
    let user = authenticate(headers).await?;
    let jwt = generate_jwt("webserver-download", &user, 10 * 60).await?;
    let response = DownloadAuthResponse { jwt };
    Ok(Json(response))
}
