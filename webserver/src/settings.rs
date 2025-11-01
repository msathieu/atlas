use crate::{auth::require_admin, get_db};
use axum::{
    Json,
    extract::Path,
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

pub async fn get_setting(headers: HeaderMap, Path(name): Path<String>) -> Result<Json<APISetting>, StatusCode> {
    require_admin(headers).await?;
    let mut db = get_db()?;
    let allowed_settings = ["allow_new_devices", "apiserver_host", "openrouter_model", "tls_cert"];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    let setting: Setting = settings::table.find(name).get_result(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(APISetting {
        name: setting.name,
        value: setting.value,
    }))
}
pub async fn update_setting(headers: HeaderMap, Path(name): Path<String>, Json(request): Json<UpdateSettingRequest>) -> Result<(), StatusCode> {
    require_admin(headers).await?;
    let mut db = get_db()?;
    let allowed_settings = ["allow_new_devices", "apiserver_host", "openrouter_model", "openrouter_key"];
    if !allowed_settings.contains(&name.as_str()) {
        return Err(StatusCode::NOT_FOUND);
    }
    diesel::update(settings::table.find(name))
        .set(settings::value.eq(request.value))
        .execute(&mut db)
        .or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
