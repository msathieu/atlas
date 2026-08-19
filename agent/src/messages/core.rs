use std::{env::current_exe, error::Error, process::Stdio, time::Duration};

use prost::Message;
use shared_api::{CommandLanguage, CommandRequest, CommandResponse, HeartbeatRequest, HeartbeatResponse, MessageType, read_message, write_message};
use tokio::{io::AsyncWriteExt, net::TcpStream, process::Command, time::timeout};
use tokio_rustls::TlsStream;

pub async fn handle_command(msg: CommandRequest, stream: &mut TlsStream<TcpStream>) -> Result<(), Box<dyn Error>> {
    let subcommand = match CommandLanguage::try_from(msg.language)? {
        CommandLanguage::Lua => "execute-lua",
        CommandLanguage::Python => "execute-python",
    };
    let mut cmd = Command::new(current_exe()?)
        .arg(subcommand)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    cmd.stdin.take().ok_or("Invalid FD")?.write_all(msg.code.as_bytes()).await?;
    while timeout(Duration::from_secs(30), async {
        cmd.wait().await.ok();
    })
    .await
    .is_err()
    {
        let random = rand::random();
        write_message(MessageType::HeartbeatRequest, HeartbeatRequest { random }, stream).await?;
        let (_, buf) = read_message(stream, Some(MessageType::HeartbeatResponse)).await?;
        let response = HeartbeatResponse::decode(&*buf)?;
        if response.random != random {
            return Err("Invalid heartbeat response".into());
        }
    }
    let result = cmd.wait_with_output().await?;
    let response = CommandResponse {
        status: *result.status.code().get_or_insert(i32::MAX) as u32,
        stdout: String::from_utf8(result.stdout)?,
        stderr: String::from_utf8(result.stderr)?,
    };
    write_message(MessageType::CommandResponse, response, stream).await?;
    Ok(())
}
