use self_replace::self_replace;
use sha2::{Digest, Sha512};
use shared_api::{MessageType, UpdateRequest, UpdateResponse, write_message};
use std::{
    error::Error,
    fs::Permissions,
    os::unix::fs::PermissionsExt,
    process::{Stdio, exit},
};
use tempfile::{NamedTempFile, TempPath};
use tokio::{fs, net::TcpStream, process::Command};
use tokio_rustls::client::TlsStream;

pub async fn handle_update(msg: UpdateRequest, stream: &mut TlsStream<TcpStream>) -> Result<(), Box<dyn Error>> {
    let tempfile = NamedTempFile::new()?.into_temp_path();
    fs::write(&tempfile, msg.agent).await?;
    fs::set_permissions(&tempfile, Permissions::from_mode(0o755)).await?;
    let file = fs::read(&tempfile).await?;
    let hash = Sha512::digest(file).to_vec();
    if hash != msg.hash {
        return Err("Invalid update hash".into());
    }
    if test_new_agent(&tempfile).await.is_err() {
        write_message(MessageType::UpdateResponse, UpdateResponse { success: false }, stream).await?;
        return Ok(());
    }
    self_replace(&tempfile)?;
    tempfile.close()?;
    write_message(MessageType::UpdateResponse, UpdateResponse { success: true }, stream).await?;
    exit(0);
}

async fn test_new_agent(path: &TempPath) -> Result<(), Box<dyn Error>> {
    let mut cmd = Command::new(path).arg("self-test").stdout(Stdio::piped()).spawn()?;
    if !cmd.wait().await?.success() {
        return Err("Self-test failed".into());
    }
    let output = cmd.wait_with_output().await?;
    if String::from_utf8(output.stdout)? != "OK\n" {
        return Err("Self-test failed".into());
    }
    Ok(())
}
