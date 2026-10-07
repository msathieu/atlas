use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Pool};
use prost::Message;
use redis::AsyncTypedCommands;
use shared_api::ForwardSshAgentRequest;
use shared_api::ForwardSshAgentResponse;
use shared_api::HeartbeatRequest;
use shared_api::MessageType;
use shared_api::handle_heartbeat;
use shared_api::read_message;
use shared_api::write_message;
use shared_server::get_valkey_client;
use shared_server::hsm::HSMOperation;
use shared_server::hsm::matches_ssh_pubkey;
use shared_server::hsm::request_hsm_operation;
use shared_server::models::Container;
use shared_server::models::DevelopmentEnvironment;
use shared_server::notify_server_with_msg;
use shared_server::schema::containers;
use shared_server::schema::development_environments;
use shared_server::wait_for_pubsub_msg;
use shared_server::{models::Device, schema::devices};
use std::error::Error;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsStream;
use uuid::Uuid;

pub async fn handle_ssh_forward(msg: ForwardSshAgentRequest, stream: &mut TlsStream<TcpStream>, environment_uuid: Uuid, uid: Uuid, pool: &Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
    let environment: DevelopmentEnvironment = development_environments::table.find(environment_uuid).first(&mut db).await?;
    if !matches_ssh_pubkey(&environment.ssh_key, &msg.key)? {
        write_message(MessageType::ForwardSSHAgentResponse, ForwardSshAgentResponse { signature: vec![], accepted: false }, stream).await?;
        return Ok(());
    }
    drop(db);
    let (mut valkey_client, mut receiver) = get_valkey_client().await?;
    let signing_id = Uuid::new_v4();
    let valkey_key = format!("SSH_FORWARD:{signing_id}");
    valkey_client.set_ex(valkey_key, environment_uuid.to_string(), 60).await.or(Err("Failed to set valkey key"))?;
    valkey_client.subscribe(format!("API:SSH_FORWARD:{signing_id}")).await?;
    notify_server_with_msg(&format!("WEB:REFRESH:{uid}"), &format!("SSH_FORWARD:{signing_id}")).await?;
    let valkey_response = match timeout(Duration::from_secs(60), async { wait_for_pubsub_msg(&mut receiver).await }).await {
        Ok(data) => data?,
        Err(_) => return Err("Timeout waiting for user approval".into()),
    };
    let mut response = ForwardSshAgentResponse { signature: vec![], accepted: false };
    if valkey_response == "APPROVED" {
        let signature = request_hsm_operation(HSMOperation::SignSSHRequest(environment.ssh_key, msg.data)).await?;
        response.signature = BASE64_STANDARD.decode(signature)?;
        response.accepted = true;
    }
    write_message(MessageType::ForwardSSHAgentResponse, response, stream).await?;
    Ok(())
}
pub async fn handle_ssh_agent(mut stream: TlsStream<TcpStream>, pool: Pool<AsyncPgConnection>, device_uuid: Uuid) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
    let device: Device = devices::table.find(device_uuid).first(&mut db).await?;
    let development_environment: DevelopmentEnvironment = development_environments::table.find(device.container_id.ok_or("Device is not a container")?).first(&mut db).await?;
    let container: Container = containers::table.find(development_environment.id).first(&mut db).await?;
    drop(db);
    loop {
        let mut db = pool.get().await?;
        let device_exists = devices::table.find(device_uuid).first::<Device>(&mut db).await.optional()?.is_some();
        drop(db);
        if !device_exists {
            return Err("Device no longer exists".into());
        }
        let (message_type, buf) = read_message(&mut stream, None).await?;
        match message_type {
            MessageType::HeartbeatRequest => handle_heartbeat(HeartbeatRequest::decode(&*buf)?, &mut stream).await,
            MessageType::ForwardSSHAgentRequest => handle_ssh_forward(ForwardSshAgentRequest::decode(&*buf)?, &mut stream, development_environment.id, container.uid, &pool).await,
            _ => return Err("Invalid message type".into()),
        }?
    }
}
