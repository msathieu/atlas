use crate::{
    auth::authenticate,
    get_db,
    util::{convert_timestamp, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use shared_api::{ContainerOrigin, ContainerState};
use shared_server::{
    models::{Container, DevelopmentEnvironment, Setting},
    notify_server,
    schema::{containers, development_environments, settings},
};
use std::{error::Error, time::SystemTime};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIDevelopmentEnvironment {
    id: Uuid,
    repo: String,
    branch: Option<String>,
    created_at: u64,
    state: String,
    pending: bool,
}
#[derive(Deserialize)]
pub struct CreateDevelopmentEnvironmentRequest {
    repo: String,
    branch: String,
}
#[derive(Serialize)]
pub struct CreateDevelopmentEnvironmentResponse {
    id: Uuid,
}

pub async fn development_environments_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APIDevelopmentEnvironment>>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db()?;
    let environments: Vec<DevelopmentEnvironment> = development_environments::table
        .inner_join(containers::table.on(development_environments::id.eq(containers::id)))
        .filter(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .select(DevelopmentEnvironment::as_select())
        .get_results(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let environments: Result<Vec<APIDevelopmentEnvironment>, StatusCode> = environments
        .iter()
        .map(|environment| {
            let container = containers::table
                .find(environment.id)
                .first::<shared_server::models::Container>(&mut db)
                .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
            let state = match ContainerState::try_from(container.state as u32).or(Err(StatusCode::INTERNAL_SERVER_ERROR))? {
                ContainerState::NonExistent => "Non-Existent",
                ContainerState::Started => "Started",
                ContainerState::Running => "Running",
                ContainerState::Stopped => "Stopped",
            }
            .to_string();
            Ok(APIDevelopmentEnvironment {
                id: environment.id,
                repo: environment.repo.clone(),
                branch: environment.branch.clone(),
                created_at: convert_timestamp(container.created_at)?,
                state,
                pending: container.requested_state != container.state,
            })
        })
        .collect();
    let environments = environments?;
    Ok((sign_response(signature, Some(&environments))?, Json(environments)))
}
pub async fn create_development_environment(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateDevelopmentEnvironmentResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateDevelopmentEnvironmentRequest = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let image: Setting = settings::table.find("development_environment_image").first(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut container = Container {
        id: None,
        created_at: SystemTime::now(),
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        origin: ContainerOrigin::DevelopmentEnvironment as i32,
        state: ContainerState::NonExistent as i32,
        requested_state: ContainerState::Running as i32,
        image: image.value,
    };
    let mut id = Uuid::nil();
    db.transaction(|db| {
        container = diesel::insert_into(containers::table).values(container).get_result(db)?;
        let branch = if request.branch.is_empty() { None } else { Some(request.branch) };
        id = container.id.ok_or("Missing ID")?;
        let development_environment = DevelopmentEnvironment { id, repo: request.repo, branch };
        diesel::insert_into(development_environments::table).values(development_environment).execute(db)?;
        Ok::<(), Box<dyn Error>>(())
    })
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateDevelopmentEnvironmentResponse { id };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn delete_development_environment(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let mut db = get_db()?;
    diesel::update(
        containers::table.filter(
            containers::id
                .eq(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                .and(containers::origin.eq(ContainerOrigin::DevelopmentEnvironment as i32)),
        ),
    )
    .set(containers::requested_state.eq(ContainerState::NonExistent as i32))
    .execute(&mut db)
    .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
