use crate::{DATABASE_URL, misc::start_connection};
use diesel::{Connection, SqliteConnection};
use prost::Message;
use shared_api::{ForwardSshAgentRequest, ForwardSshAgentResponse, HandshakePurpose, MessageType, StartSshAgentRequest, StartSshAgentResponse, read_message, send_heartbeat, write_message};
use ssh_agent_lib::{
    agent::{Session, listen},
    async_trait,
    error::AgentError,
    proto::{Identity, PublicCredential, SignRequest},
};
use ssh_key::{PublicKey, Signature};
use std::{
    error::Error,
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tokio::{
    fs::{remove_file, set_permissions},
    net::{TcpStream, UnixListener},
    select,
    sync::{Mutex, Notify},
    task::JoinHandle,
    time::sleep,
};
use tokio_rustls::{TlsConnector, TlsStream};

#[derive(Clone)]
struct SSHAgent {
    key: PublicKey,
    stream: Arc<Mutex<TlsStream<TcpStream>>>,
}

static SSH_TASK: LazyLock<Mutex<Option<JoinHandle<()>>>> = LazyLock::new(|| Mutex::new(None));
static SSH_HEALTH_NOTIFY: Notify = Notify::const_new();

async fn sign_internal(agent: &mut SSHAgent, request: SignRequest) -> Result<Option<Signature>, Box<dyn Error>> {
    let mut stream = agent.stream.lock().await;
    write_message(
        MessageType::ForwardSSHAgentRequest,
        ForwardSshAgentRequest {
            data: request.data,
            key: agent.key.to_openssh()?,
        },
        &mut *stream,
    )
    .await?;
    let (_, buf) = read_message(&mut *stream, Some(MessageType::ForwardSSHAgentResponse)).await?;
    let response = ForwardSshAgentResponse::decode(&*buf)?;
    if !response.accepted {
        return Ok(None);
    }
    let signature = Signature::new(agent.key.algorithm(), response.signature)?;
    Ok(Some(signature))
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
        let signature = match sign_internal(self, request).await {
            Ok(signature) => signature,
            Err(_) => {
                SSH_HEALTH_NOTIFY.notify_one();
                return Err(AgentError::Failure);
            }
        };
        match signature {
            Some(signature) => Ok(signature),
            None => Err(AgentError::Failure),
        }
    }
}

async fn check_health() -> Result<(), Box<dyn Error>> {
    SSH_HEALTH_NOTIFY.notified().await;
    Err("SSH Agent is unhealthy".into())
}
async fn heartbeat_loop(stream: Arc<Mutex<TlsStream<TcpStream>>>) -> Result<(), Box<dyn Error>> {
    loop {
        sleep(Duration::from_secs(60)).await;
        let mut stream_locked = stream.lock().await;
        send_heartbeat(&mut stream_locked).await?;
    }
}
async fn ssh_agent(key: PublicKey, host: &str, tls_connector: &TlsConnector, db: &mut SqliteConnection) -> Result<(), Box<dyn Error>> {
    let stream = start_connection(host, tls_connector, db, HandshakePurpose::SSHAgent).await?.ok_or("Invalid device state")?;
    let path = "/run/ssh.sock";
    remove_file(path).await.ok();
    let listener = UnixListener::bind(path)?;
    set_permissions(path, Permissions::from_mode(0o777)).await?;
    let stream = Arc::new(Mutex::new(stream));
    let agent = SSHAgent { key, stream: stream.clone() };
    select! {
        result = listen(listener, agent) => result?,
        result = check_health() => result?,
        result = heartbeat_loop(stream) => result?,
    };
    Ok(())
}
async fn ssh_agent_task(key: PublicKey, host: String, tls_connector: TlsConnector) -> Result<(), Box<dyn Error>> {
    let mut db = SqliteConnection::establish(DATABASE_URL)?;
    loop {
        if let Err(e) = ssh_agent(key.clone(), &host, &tls_connector, &mut db).await {
            eprintln!("Error in connection: {}", e);
        }
        sleep(Duration::from_secs(5)).await;
    }
}
pub async fn handle_ssh_agent(msg: StartSshAgentRequest, stream: &mut TlsStream<TcpStream>, host: String, tls_connector: TlsConnector) -> Result<(), Box<dyn Error>> {
    let mut ssh_task_slot = SSH_TASK.lock().await;
    if let Some(ssh_task) = ssh_task_slot.take() {
        ssh_task.abort();
        let _ = ssh_task.await;
    }
    let public_key = PublicKey::from_openssh(&msg.key)?;
    let task = tokio::spawn(async {
        ssh_agent_task(public_key, host, tls_connector).await.unwrap();
    });
    *ssh_task_slot = Some(task);
    write_message(MessageType::StartSSHAgentResponse, StartSshAgentResponse {}, stream).await?;
    Ok(())
}
