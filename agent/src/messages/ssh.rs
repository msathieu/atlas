use shared_api::{MessageType, SshAgentRequest, SshAgentResponse, write_message};
use signature::Signer;
use ssh_agent_lib::{
    agent::{Session, listen},
    async_trait,
    error::AgentError,
    proto::{Identity, PublicCredential, SignRequest},
};
use ssh_key::{PrivateKey, Signature};
use std::{error::Error, fs::Permissions, os::unix::fs::PermissionsExt};
use tokio::{
    fs::{remove_file, set_permissions},
    net::{TcpStream, UnixListener},
};
use tokio_rustls::client::TlsStream;

#[derive(Clone)]
struct SSHAgent {
    key: PrivateKey,
}

#[async_trait]
impl Session for SSHAgent {
    async fn request_identities(&mut self) -> Result<Vec<Identity>, AgentError> {
        Ok(vec![Identity {
            credential: PublicCredential::Key(self.key.public_key().key_data().clone()),
            comment: self.key.comment().to_string(),
        }])
    }
    async fn sign(&mut self, request: SignRequest) -> Result<Signature, AgentError> {
        if request.credential.key_data() != self.key.public_key().key_data() {
            return Err(AgentError::Failure);
        }
        self.key.try_sign(&request.data).or(Err(AgentError::Failure))
    }
}

pub async fn handle_ssh_agent(msg: SshAgentRequest, stream: &mut TlsStream<TcpStream>) -> Result<(), Box<dyn Error>> {
    let path = "/run/ssh.sock";
    remove_file(path).await.ok();
    let listener = UnixListener::bind(path)?;
    set_permissions(path, Permissions::from_mode(0o777)).await?;
    let private_key = PrivateKey::from_openssh(msg.key)?;
    let agent = SSHAgent { key: private_key };
    tokio::spawn(async {
        listen(listener, agent).await.unwrap();
    });
    write_message(MessageType::SSHAgentResponse, SshAgentResponse {}, stream).await?;
    Ok(())
}
