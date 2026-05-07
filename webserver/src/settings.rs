use crate::{
    auth::require_admin,
    get_db,
    util::{decrypt_body, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use shared_server::{models::Setting, notify_server, schema::settings};

#[derive(Serialize)]
pub struct APISetting {
    name: String,
    value: String,
}
#[derive(Deserialize)]
pub struct UpdateSettingRequest {
    value: String,
}

pub async fn get_setting(headers: HeaderMap, Path(name): Path<String>, uri: OriginalUri, body: Bytes) -> Result<Json<APISetting>, StatusCode> {
    require_admin(&headers)?;
    validate_signature(&headers, uri.0, &body, "GET")?;
    let mut db = get_db()?;
    let allowed_settings = [
        "allow_new_devices",
        "apiserver_host",
        "mail_server",
        "mail_username",
        "mail_from",
        "mail_to",
        "openrouter_model",
        "tls_cert",
    ];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    let setting: Setting = settings::table.find(name).get_result(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(APISetting {
        name: setting.name,
        value: setting.value,
    }))
}
pub async fn update_setting(headers: HeaderMap, Path(name): Path<String>, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    require_admin(&headers)?;
    validate_signature(&headers, uri.0, &body, "PUT")?;
    let request: UpdateSettingRequest = serde_json::from_str(&decrypt_body(body)?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let allowed_settings = [
        "allow_new_devices",
        "apiserver_host",
        "mail_server",
        "mail_username",
        "mail_password",
        "mail_from",
        "mail_to",
        "openrouter_model",
        "openrouter_key",
    ];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    diesel::update(settings::table.find(&name))
        .set(settings::value.eq(request.value))
        .execute(&mut db)
        .or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
pub async fn restart_system(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    require_admin(&headers)?;
    validate_signature(&headers, uri.0, &body, "POST")?;
    notify_server("MASTER:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("API:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:RESTART").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
