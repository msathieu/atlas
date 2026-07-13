#![forbid(unsafe_code)]

use crate::{
    agent::{download_agent, download_auth, get_agent_script},
    auth::login,
    chat::{chats_list, create_chat, delete_chat, delete_message, get_chat, manage_tool_call_consent, send_message},
    commands::{commands_list, create_command, delete_command},
    development_environments::{
        create_development_environment, delete_development_environment, development_environments_list, open_development_environment, start_development_environment, stop_development_environment,
    },
    devices::{approve_device, delete_device, devices_list},
    info::{assetlinks, server_info},
    passkey::{delete_passkey, finish_passkey_authentication, finish_passkey_registration, passkeys_list, start_passkey_authentication, start_passkey_registration},
    settings::{get_settings, restart_system, update_setting},
    stream::{stream, stream_auth},
    user::{change_password, get_user_info},
    users::{create_user, delete_user, users_list},
};
use axum::{
    Router,
    http::StatusCode,
    routing::{delete, get, post, put},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::{
    PgConnection,
    r2d2::{Pool, PooledConnection},
};
use diesel::{prelude::*, r2d2::ConnectionManager};
use shared_api::print_startup;
use shared_server::{get_db_pool, listen_for_restarts, models::Setting, schema};
use std::sync::OnceLock;
use tokio::net::TcpListener;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};

mod agent;
mod auth;
mod chat;
mod commands;
mod development_environments;
mod devices;
mod info;
mod passkey;
mod settings;
mod stream;
mod user;
mod users;
mod util;

static JWT_KEY: OnceLock<Vec<u8>> = OnceLock::new();
static ENCRYPTION_KEY: OnceLock<Vec<u8>> = OnceLock::new();
static SIGNING_KEY: OnceLock<Vec<u8>> = OnceLock::new();
static DB_POOL: OnceLock<Pool<ConnectionManager<PgConnection>>> = OnceLock::new();

fn get_db() -> Result<PooledConnection<ConnectionManager<PgConnection>>, StatusCode> {
    DB_POOL.get().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?.get().or(Err(StatusCode::INTERNAL_SERVER_ERROR))
}

#[tokio::main]
async fn main() {
    print_startup("webserver", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    listen_for_restarts("WEB".to_string()).await;
    DB_POOL.set(get_db_pool().unwrap()).unwrap();
    let jwt_secret = schema::settings::table.find("jwt_secret").first::<Setting>(&mut DB_POOL.get().unwrap().get().unwrap()).unwrap().value;
    let jwt_secret = BASE64_STANDARD.decode(jwt_secret).unwrap();
    JWT_KEY.set(jwt_secret).unwrap();
    let encryption_key = schema::settings::table
        .find("web_encryption_key")
        .first::<Setting>(&mut DB_POOL.get().unwrap().get().unwrap())
        .unwrap()
        .value;
    let encryption_key = BASE64_STANDARD.decode(encryption_key).unwrap();
    ENCRYPTION_KEY.set(encryption_key).unwrap();
    let signing_key = schema::settings::table
        .find("web_signing_key")
        .first::<Setting>(&mut DB_POOL.get().unwrap().get().unwrap())
        .unwrap()
        .value;
    let signing_key = BASE64_STANDARD.decode(signing_key).unwrap();
    SIGNING_KEY.set(signing_key).unwrap();
    let router = Router::new()
        .fallback_service(ServeDir::new("frontend").fallback(ServeFile::new("frontend/404.html")))
        .route("/api/info", get(server_info))
        .route("/api/devices", get(devices_list))
        .route("/api/devices/{id}/approve", post(approve_device))
        .route("/api/devices/{id}", delete(delete_device))
        .route("/api/commands", get(commands_list))
        .route("/api/commands", post(create_command))
        .route("/api/commands/{id}", delete(delete_command))
        .route("/api/chats", get(chats_list))
        .route("/api/chats", post(create_chat))
        .route("/api/chats/{id}", delete(delete_chat))
        .route("/api/chats/{id}", get(get_chat))
        .route("/api/chats/{id}", post(send_message))
        .route("/api/chats/{id}/{seq}", delete(delete_message))
        .route("/api/chats/{id}/{seq}/{call_id}/consent", post(manage_tool_call_consent))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/{name}", put(update_setting))
        .route("/api/login", post(login))
        .route("/api/stream", get(stream))
        .route("/api/stream_auth", post(stream_auth))
        .route("/api/atlas-agent", get(download_agent))
        .route("/api/download_auth", post(download_auth))
        .route("/api/agent_script", get(get_agent_script))
        .route("/api/restart", post(restart_system))
        .route("/api/user/update_password", post(change_password))
        .route("/api/user/info", get(get_user_info))
        .route("/api/users", get(users_list))
        .route("/api/users/{id}", delete(delete_user))
        .route("/api/users", post(create_user))
        .route("/api/passkeys/start_registration", post(start_passkey_registration))
        .route("/api/passkeys/finish_registration", post(finish_passkey_registration))
        .route("/api/passkeys/start_authentication", post(start_passkey_authentication))
        .route("/api/passkeys/finish_authentication", post(finish_passkey_authentication))
        .route("/api/passkeys", get(passkeys_list))
        .route("/api/passkeys/{id}", delete(delete_passkey))
        .route("/.well-known/assetlinks.json", get(assetlinks))
        .route("/api/development_environments", get(development_environments_list))
        .route("/api/development_environments", post(create_development_environment))
        .route("/api/development_environments/{id}", delete(delete_development_environment))
        .route("/api/development_environments/{id}/open", post(open_development_environment))
        .route("/api/development_environments/{id}/start", post(start_development_environment))
        .route("/api/development_environments/{id}/stop", post(stop_development_environment))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any).expose_headers(Any));
    let listener = TcpListener::bind("0.0.0.0:8000").await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
