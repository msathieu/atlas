use crate::{auth::authenticate, get_db, util::convert_timestamp};
use axum::{
    Json,
    extract::Path,
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use shared_api::CommandOrigin;
use shared_server::{models::Command, notify_server, schema::commands};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Serialize)]
pub struct APICommand {
    id: Uuid,
    device: Uuid,
    code: String,
    completed: bool,
    status_code: i32,
    stdout: String,
    stderr: String,
    requested_timestamp: u64,
    completed_timestamp: Option<u64>,
}
#[derive(Deserialize)]
pub struct CreateCommandRequest {
    device: Uuid,
    code: String,
}
#[derive(Serialize)]
pub struct CreateCommandResponse {
    id: Uuid,
}

pub async fn commands_list(headers: HeaderMap) -> Result<Json<Vec<APICommand>>, StatusCode> {
    let user = authenticate(headers).await?;
    let mut db = get_db()?;
    let commands: Vec<Command> = commands::table
        .filter(
            commands::origin
                .eq(CommandOrigin::UserRequested as i32)
                .and(commands::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        )
        .get_results(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let commands: Result<Vec<APICommand>, StatusCode> = commands
        .iter()
        .map(|command| {
            Ok(APICommand {
                id: command.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
                device: command.device,
                code: command.code.clone(),
                completed: command.completed,
                status_code: command.status_code,
                stdout: command.stdout.clone(),
                stderr: command.stderr.clone(),
                requested_timestamp: convert_timestamp(command.requested_timestamp)?,
                completed_timestamp: command.completed_timestamp.map(|timestamp| convert_timestamp(timestamp).unwrap_or(0)),
            })
        })
        .collect();
    Ok(Json(commands?))
}
pub async fn create_command(headers: HeaderMap, Json(request): Json<CreateCommandRequest>) -> Result<Json<CreateCommandResponse>, StatusCode> {
    let user = authenticate(headers).await?;
    let mut db = get_db()?;
    let mut cmd = Command {
        id: None,
        device: request.device,
        code: request.code,
        completed: false,
        status_code: 0,
        stdout: "".to_owned(),
        stderr: "".to_owned(),
        requested_timestamp: SystemTime::now(),
        completed_timestamp: None,
        origin: CommandOrigin::UserRequested as i32,
        device_info_type: None,
        uid: Some(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?),
        initiated: false,
        chat_id: None,
        chat_sequence: None,
        chat_tool_call_id: None,
    };
    cmd = diesel::insert_into(commands::table).values(cmd).get_result(&mut db).or(Err(StatusCode::BAD_REQUEST))?;
    notify_server(&format!("API:DEVICE:{}", request.device)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(CreateCommandResponse {
        id: cmd.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    }))
}
pub async fn delete_command(headers: HeaderMap, Path(id): Path<String>) -> Result<(), StatusCode> {
    let user = authenticate(headers).await?;
    let mut db = get_db()?;
    diesel::delete(
        commands::table.filter(
            commands::id
                .eq(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?)
                .and(commands::origin.eq(CommandOrigin::UserRequested as i32))
                .and(commands::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        ),
    )
    .execute(&mut db)
    .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
