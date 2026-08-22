use crate::{
    auth::{authenticate, generate_jwt},
    get_db,
    util::{convert_timestamp, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path, Query},
    http::{HeaderMap, StatusCode},
};
use diesel::prelude::*;
use redis::{AsyncTypedCommands, ExistenceCheck, SetExpiry, SetOptions};
use serde::{Deserialize, Serialize};
use shared_api::{ContainerOrigin, ContainerState};
use shared_server::{
    get_valkey_client,
    models::{Command, Container, DevelopmentEnvironment},
    notify_server,
    schema::{commands, containers, development_environments},
};
use ssh_key::{Algorithm::Ed25519, LineEnding, PrivateKey, rand_core::OsRng};
use std::{env, error::Error, time::SystemTime};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIDevelopmentEnvironment {
    id: Uuid,
    name: String,
    repo: Option<String>,
    branch: Option<String>,
    created_at: u64,
    state: String,
    pending: bool,
    nested_containers: bool,
    ssh_key: String,
}
#[derive(Deserialize)]
pub struct CreateDevelopmentEnvironmentRequest {
    name: String,
    repo: String,
    branch: String,
    nested_containers: bool,
}
#[derive(Serialize)]
pub struct CreateDevelopmentEnvironmentResponse {
    id: Uuid,
}
#[derive(Serialize)]
pub struct OpenDevelopmentEnvironmentResponse {
    url: String,
    ready: bool,
}
#[derive(Deserialize)]
pub struct OpenDevelopmentEnvironmentRequest {
    port: Option<u16>,
}
#[derive(Serialize)]
pub struct GetSSHForwardRequestResponse {
    development_environment_id: Uuid,
    name: String,
}
#[derive(Deserialize)]
pub struct ApproveSSHForwardRequestRequest {
    approved: bool,
}

pub async fn development_environments_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APIDevelopmentEnvironment>>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db()?;
    let environments: Vec<(DevelopmentEnvironment, Container)> = development_environments::table
        .inner_join(containers::table.on(development_environments::id.eq(containers::id)))
        .filter(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .select((DevelopmentEnvironment::as_select(), Container::as_select()))
        .get_results(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let environments: Result<Vec<APIDevelopmentEnvironment>, StatusCode> = environments
        .iter()
        .map(|(environment, container)| {
            let state = match ContainerState::try_from(container.state as u32).or(Err(StatusCode::INTERNAL_SERVER_ERROR))? {
                ContainerState::NonExistent => "Non-Existent",
                ContainerState::Started => "Started",
                ContainerState::Running => "Running",
                ContainerState::Stopped => "Stopped",
            }
            .to_string();
            let mut private_key = PrivateKey::from_openssh(&environment.ssh_key).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
            private_key.set_comment(format!("{}@atlas-{}", user.username, environment.name));
            Ok(APIDevelopmentEnvironment {
                id: environment.id,
                name: environment.name.clone(),
                repo: environment.repo.clone(),
                branch: environment.branch.clone(),
                created_at: convert_timestamp(container.created_at)?,
                state,
                pending: container.requested_state != container.state,
                nested_containers: environment.nested_containers,
                ssh_key: private_key.public_key().to_openssh().or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
            })
        })
        .collect();
    let environments = environments?;
    Ok((sign_response(signature, Some(&environments))?, Json(environments)))
}
pub async fn create_development_environment(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreateDevelopmentEnvironmentResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: CreateDevelopmentEnvironmentRequest = serde_json::from_slice(&body).or(Err(StatusCode::BAD_REQUEST))?;
    if request.name.is_empty()
        || !request.name.chars().all(|c| c.is_ascii_alphanumeric())
        || !request.repo.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':' || c == '/' || c == '.')
        || !request.branch.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '/' || c == '.')
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !request.repo.is_empty() && !request.repo.starts_with("https://") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if request.nested_containers && !user.admin {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut db = get_db()?;
    let image = env::var("IMAGE_IDE").or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut container = Container {
        id: None,
        created_at: SystemTime::now(),
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        origin: ContainerOrigin::DevelopmentEnvironment as i32,
        state: ContainerState::NonExistent as i32,
        requested_state: ContainerState::Stopped as i32,
        image,
        storage_size: 25,
        memory_limit: 16,
    };
    let mut id = Uuid::nil();
    let repo = if request.repo.is_empty() { None } else { Some(request.repo) };
    let branch = if request.branch.is_empty() { None } else { Some(request.branch) };
    let ssh_key = PrivateKey::random(&mut OsRng, Ed25519).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let ssh_key_exported = ssh_key.to_openssh(LineEnding::LF).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    db.transaction(|db| {
        container = diesel::insert_into(containers::table).values(container).get_result(db)?;
        id = container.id.ok_or("Missing ID")?;
        let development_environment = DevelopmentEnvironment {
            id,
            name: request.name,
            repo,
            branch,
            command: None,
            nested_containers: request.nested_containers,
            ssh_key: ssh_key_exported.to_string(),
        };
        diesel::insert_into(development_environments::table).values(development_environment).execute(db)?;
        Ok::<(), Box<dyn Error>>(())
    })
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = CreateDevelopmentEnvironmentResponse { id };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn delete_development_environment(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let mut db = get_db()?;
    diesel::update(
        containers::table.filter(
            containers::id
                .eq(id)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                .and(containers::origin.eq(ContainerOrigin::DevelopmentEnvironment as i32)),
        ),
    )
    .set(containers::requested_state.eq(ContainerState::NonExistent as i32))
    .execute(&mut db)
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn start_development_environment(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let mut db = get_db()?;
    containers::table
        .filter(
            containers::id
                .eq(id)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                .and(containers::origin.eq(ContainerOrigin::DevelopmentEnvironment as i32)),
        )
        .first::<Container>(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    let key = format!("DEVELOPMENT_ENVIRONMENT:{}", id);
    get_valkey_client()
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
        .0
        .set_ex(key, true, 16 * 60 * 60)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    diesel::update(
        containers::table.filter(
            containers::id
                .eq(id)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                .and(containers::origin.eq(ContainerOrigin::DevelopmentEnvironment as i32)),
        ),
    )
    .set(containers::requested_state.eq(ContainerState::Running as i32))
    .execute(&mut db)
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn stop_development_environment(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let mut db = get_db()?;
    diesel::update(
        containers::table.filter(
            containers::id
                .eq(id)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
                .and(containers::origin.eq(ContainerOrigin::DevelopmentEnvironment as i32)),
        ),
    )
    .set(containers::requested_state.eq(ContainerState::Stopped as i32))
    .execute(&mut db)
    .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:CONTAINERS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn open_development_environment(
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    uri: OriginalUri,
    request: Query<OpenDevelopmentEnvironmentRequest>,
    body: Bytes,
) -> Result<(HeaderMap, Json<OpenDevelopmentEnvironmentResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let mut db = get_db()?;
    let container: Container = containers::table
        .filter(
            containers::uid
                .eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)
                .and(containers::origin.eq(ContainerOrigin::DevelopmentEnvironment as i32))
                .and(containers::id.eq(id)),
        )
        .first(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    let mut ready = true;
    if container.state != ContainerState::Running as i32 {
        ready = false;
    }
    let development_environment: DevelopmentEnvironment = development_environments::table.find(id).first(&mut db).or(Err(StatusCode::NOT_FOUND))?;
    if let Some(command) = development_environment.command {
        let command: Command = commands::table.find(command).first(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        if !command.completed {
            ready = false;
        }
    } else {
        ready = false;
    }
    let webserver = env::var("WEBSERVER_HOST").or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let jwt = match request.port {
        None => generate_jwt(&format!("proxy-dev-{}", id), &user, 12 * 60 * 60, None)?,
        Some(_) => generate_jwt(&format!("proxy-devproxy-{}", id), &user, 12 * 60 * 60, None)?,
    };
    let url = match request.port {
        None => format!("https://{}.dev.{}/atlas_setup?jwt={}", id, webserver, jwt),
        Some(port) => format!("https://{}-{}.devproxy.{}/atlas_setup?jwt={}", id, port, webserver, jwt),
    };
    let response = OpenDevelopmentEnvironmentResponse { url, ready };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn get_ssh_forward_request(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<GetSSHForwardRequestResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let mut db = get_db()?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let development_environment_id: String = valkey_client
        .get(format!("SSH_FORWARD:{}", id))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
        .ok_or(StatusCode::NOT_FOUND)?;
    let development_environment: DevelopmentEnvironment = development_environments::table
        .find(Uuid::parse_str(&development_environment_id).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?)
        .first(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    containers::table
        .filter(containers::id.eq(development_environment.id).and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)))
        .first::<Container>(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    let requested_key = format!("SSH_FORWARD_REQUESTED:{}", id);
    let exists = valkey_client
        .set_options(requested_key, "", SetOptions::default().conditional_set(ExistenceCheck::NX).with_expiration(SetExpiry::EX(60)))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    if exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }
    let response = GetSSHForwardRequestResponse {
        development_environment_id: development_environment.id,
        name: development_environment.name,
    };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn approve_ssh_forward_request(headers: HeaderMap, Path(id): Path<Uuid>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: ApproveSSHForwardRequestRequest = serde_json::from_slice(&body).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let development_environment_id: String = valkey_client
        .get(format!("SSH_FORWARD:{}", id))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
        .ok_or(StatusCode::NOT_FOUND)?;
    containers::table
        .filter(
            containers::id
                .eq(Uuid::parse_str(&development_environment_id).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?)
                .and(containers::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        )
        .first::<Container>(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    let data = if request.approved { "APPROVED" } else { "DENIED" };
    let valkey_key = format!("SSH_FORWARD_APPROVAL:{}", id);
    valkey_client.set_ex(valkey_key, data, 15).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("API:SSH_FORWARD:{}", id)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
