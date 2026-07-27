use diesel::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::r2d2::PooledConnection;
use prost::Message;
use shared_api::ForwardSshAgentRequest;
use shared_api::ForwardSshAgentResponse;
use shared_api::HeartbeatRequest;
use shared_api::MessageType;
use shared_api::handle_heartbeat;
use shared_api::read_message;
use shared_api::write_message;
use shared_server::models::DevelopmentEnvironment;
use shared_server::schema::development_environments;
use shared_server::{models::Device, schema::devices};
use signature::Signer;
use ssh_key::PrivateKey;
use std::error::Error;
use tokio::net::TcpStream;
use tokio_rustls::TlsStream;
use uuid::Uuid;

pub async fn handle_ssh_forward(msg: ForwardSshAgentRequest, stream: &mut TlsStream<TcpStream>, key: &PrivateKey) -> Result<(), Box<dyn Error>> {
    let signature = key.try_sign(&msg.data)?;
    let response = ForwardSshAgentResponse {
        signature: signature.as_bytes().to_vec(),
    };
    write_message(MessageType::ForwardSSHAgentResponse, response, stream).await?;
    Ok(())
}
pub async fn handle_ssh_agent(mut stream: TlsStream<TcpStream>, mut db: PooledConnection<ConnectionManager<PgConnection>>, uuid: Uuid) -> Result<(), Box<dyn Error>> {
    let device: Device = devices::table.find(uuid).first(&mut db)?;
    let development_environment: DevelopmentEnvironment = development_environments::table.find(device.container_id.ok_or("Device is not a container")?).first(&mut db)?;
    let private_key = PrivateKey::from_openssh(development_environment.ssh_key)?;
    loop {
        let device_exists = devices::table.find(uuid).first::<Device>(&mut db).optional()?.is_some();
        if !device_exists {
            return Err("Device no longer exists".into());
        }
        let (message_type, buf) = read_message(&mut stream, None).await?;
        match message_type {
            MessageType::HeartbeatRequest => handle_heartbeat(HeartbeatRequest::decode(&*buf)?, &mut stream).await,
            MessageType::ForwardSSHAgentRequest => handle_ssh_forward(ForwardSshAgentRequest::decode(&*buf)?, &mut stream, &private_key).await,
            _ => return Err("Invalid message type".into()),
        }?
    }
}
