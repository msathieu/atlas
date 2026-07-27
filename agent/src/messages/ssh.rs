use crate::{DATABASE_URL, misc::start_connection};
use diesel::{Connection, SqliteConnection};
use prost::Message;
use shared_api::{ForwardSshAgentRequest, ForwardSshAgentResponse, HandshakePurpose, MessageType, StartSshAgentRequest, StartSshAgentResponse, read_message, write_message};
use ssh_agent_lib::{
    agent::{Session, listen},
    async_trait,
    error::AgentError,
    proto::{Identity, PublicCredential, SignRequest},
};
use ssh_key::{Algorithm, PublicKey, Signature};
use std::{error::Error, fs::Permissions, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    fs::{remove_file, set_permissions},
    net::{TcpStream, UnixListener},
    sync::Mutex,
    time::sleep,
};
use tokio_rustls::{TlsConnector, TlsStream};

#[derive(Clone)]
struct SSHAgent {
    key: PublicKey,
    stream: Arc<Mutex<TlsStream<TcpStream>>>,
}

#[async_trait]
impl Session for SSHAgent {
    async fn request_identities(&mut self) -> Result<Vec<Identity>, AgentError> {
        Ok(vec![Identity {
            credential: PublicCredential::Key(self.key.key_data().clone()),
            comment: self.key.comment().to_string(),
        }])
    }
    async fn sign(&mut self, request: SignRequest) -> Result<Signature, AgentError> {
        if request.credential.key_data() != self.key.key_data() {
            return Err(AgentError::Failure);
        }
        let mut stream = self.stream.lock().await;
        write_message(MessageType::ForwardSSHAgentRequest, ForwardSshAgentRequest { data: request.data }, &mut *stream)
            .await
            .or(Err(AgentError::Failure))?;
        let (_, buf) = read_message(&mut *stream, Some(MessageType::ForwardSSHAgentResponse)).await.or(Err(AgentError::Failure))?;
        let response = ForwardSshAgentResponse::decode(&*buf).or(Err(AgentError::Failure))?;
        let signature = Signature::new(Algorithm::Ed25519, response.signature).or(Err(AgentError::Failure))?;
        Ok(signature)
    }
}

async fn ssh_agent(key: PublicKey, host: &str, tls_connector: &TlsConnector, db: &mut SqliteConnection) -> Result<(), Box<dyn Error>> {
    let stream = start_connection(host, tls_connector, db, HandshakePurpose::SSHAgent).await?.ok_or("Invalid device state")?;
    let path = "/run/ssh.sock";
    remove_file(path).await.ok();
    let listener = UnixListener::bind(path)?;
    set_permissions(path, Permissions::from_mode(0o777)).await?;
    let agent = SSHAgent {
        key,
        stream: Arc::new(Mutex::new(stream)),
    };
    listen(listener, agent).await?;
    Ok(())
}
async fn ssh_agent_task(key: PublicKey, host: String, tls_connector: TlsConnector) -> Result<(), Box<dyn Error>> {
    let mut db = SqliteConnection::establish(DATABASE_URL)?;
    loop {
        if let Err(e) = ssh_agent(key.clone(), &host, &tls_connector, &mut db).await {
            eprintln!("Error in connection: {}", e);
        }
        sleep(Duration::from_secs(10)).await;
    }
}
pub async fn handle_ssh_agent(msg: StartSshAgentRequest, stream: &mut TlsStream<TcpStream>, host: String, tls_connector: TlsConnector) -> Result<(), Box<dyn Error>> {
    let public_key = PublicKey::from_openssh(&msg.key)?;
    tokio::spawn(async {
        ssh_agent_task(public_key, host, tls_connector).await.unwrap();
    });
    write_message(MessageType::StartSSHAgentResponse, StartSshAgentResponse {}, stream).await?;
    Ok(())
}
