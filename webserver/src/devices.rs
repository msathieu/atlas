use crate::{
    auth::{authenticate, require_admin},
    get_db,
    util::convert_timestamp,
};
use axum::{
    Json,
    extract::Path,
    http::{HeaderMap, StatusCode},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use serde::Serialize;
use shared_server::{models::Device, notify_server, schema::devices};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIDevice {
    id: Uuid,
    arch: String,
    os: String,
    version: String,
    pubkey: String,
    approved: bool,
    hostname: String,
    first_connection: u64,
    last_connection: u64,
    ip: String,
}

pub async fn devices_list(headers: HeaderMap) -> Result<Json<Vec<APIDevice>>, StatusCode> {
    authenticate(headers).await?;
    let mut db = get_db()?;
    let devices: Vec<Device> = devices::table.get_results(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let devices: Result<Vec<APIDevice>, StatusCode> = devices
        .iter()
        .map(|device| {
            Ok(APIDevice {
                id: device.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
                arch: device.arch.clone(),
                os: device.os.clone(),
                version: device.version.clone(),
                pubkey: BASE64_STANDARD.encode(&device.pubkey),
                approved: device.approved,
                hostname: device.hostname.clone(),
                first_connection: convert_timestamp(device.first_connection)?,
                last_connection: convert_timestamp(device.last_connection)?,
                ip: device.ip.clone(),
            })
        })
        .collect();
    Ok(Json(devices?))
}
pub async fn approve_device(headers: HeaderMap, Path(id): Path<String>) -> Result<(), StatusCode> {
    require_admin(headers).await?;
    let mut db = get_db()?;
    diesel::update(devices::table.find(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?))
        .set(devices::approved.eq(true))
        .execute(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("WEB").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:DEVICES").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
pub async fn delete_device(headers: HeaderMap, Path(id): Path<String>) -> Result<(), StatusCode> {
    require_admin(headers).await?;
    let mut db = get_db()?;
    diesel::delete(devices::table.find(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?))
        .execute(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("WEB").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
