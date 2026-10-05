#![forbid(unsafe_code)]

use num_enum::TryFromPrimitive;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::{env, error::Error, fmt};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::TlsStream;

include!(concat!(env!("OUT_DIR"), "/agent.rs"));

pub const MAGIC: usize = 13609285118649036194;
pub const API_VERSION: usize = 1;
pub const SUPPORTED_ARCH: [&str; 1] = ["x86_64"];
pub const SUPPORTED_OS: [&str; 1] = ["linux"];

#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum MessageType {
    HandshakeRequest = 1,
    HandshakeResponse = 2,
    HandshakeFinish = 3,
    HeartbeatRequest = 4,
    HeartbeatResponse = 5,
    CommandRequest = 6,
    CommandResponse = 7,
    UpdateRequest = 8,
    UpdateResponse = 9,
    StartSSHAgentRequest = 10,
    StartSSHAgentResponse = 11,
    ForwardSSHAgentRequest = 12,
    ForwardSSHAgentResponse = 13,
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum CommandOrigin {
    SystemIdentification = 0,
    UserRequested = 1,
    LLMTool = 2,
    SystemInitiated = 3,
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum CommandLanguage {
    Lua = 0,
    Python = 1,
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum ChatType {
    Interactive = 0,
    NonInteractive = 1,
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum ContainerOrigin {
    DevelopmentEnvironment = 0,
    Agent = 1,
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum ContainerState {
    NonExistent = 0,
    Stopped = 1,
    Started = 2,
    Running = 3,
    Restarting = 4,
}
impl fmt::Display for ContainerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            ContainerState::NonExistent => "Non-Existent",
            ContainerState::Started => "Started",
            ContainerState::Running => "Running",
            ContainerState::Stopped => "Stopped",
            ContainerState::Restarting => "Restarting",
        };
        write!(f, "{string}")
    }
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum HandshakePurpose {
    General = 0,
    SSHAgent = 1,
}
#[derive(PartialEq, TryFromPrimitive, Deserialize, Serialize)]
#[repr(u32)]
pub enum CryptoBackend {
    Software = 0,
    TPM = 1,
}
impl fmt::Display for CryptoBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            CryptoBackend::Software => "Software",
            CryptoBackend::TPM => "TPM",
        };
        write!(f, "{string}")
    }
}

pub fn print_startup(component: &str, version: &str) {
    println!("--------------------------------------------------");
    println!("Starting Atlas {} {}", component, version);
    if cfg!(debug_assertions) {
        println!("Running in debug mode, this is INSECURE");
    }
    println!("--------------------------------------------------");
}
pub async fn write_message<T: Message, U: AsyncWrite + Unpin>(message_type: MessageType, message: T, stream: &mut U) -> Result<(), Box<dyn Error>> {
    let bytes = message.encode_to_vec();
    let message_type = message_type as u32;
    let len = bytes.len() as u64;
    stream.write_all(&message_type.to_le_bytes()).await?;
    stream.write_all(&len.to_le_bytes()).await?;
    stream.write_all(&bytes).await?;
    Ok(())
}
pub async fn read_message<T: AsyncRead + Unpin>(stream: &mut T, message_type_requested: Option<MessageType>) -> Result<(MessageType, Vec<u8>), Box<dyn Error>> {
    let mut buf: [u8; 4] = [0; 4];
    stream.read_exact(&mut buf).await?;
    let message_type = MessageType::try_from(u32::from_le_bytes(buf))?;
    if let Some(message_type_requested) = message_type_requested
        && message_type != message_type_requested
    {
        return Err("Invalid message type received".into());
    }
    let mut buf: [u8; 8] = [0; 8];
    stream.read_exact(&mut buf).await?;
    let len = u64::from_le_bytes(buf) as usize;
    if len > 128 * 1024 * 1024 {
        return Err("Message too large".into());
    }
    if (message_type == MessageType::HandshakeRequest || message_type == MessageType::HandshakeResponse || message_type == MessageType::HandshakeFinish) && len > 1024 * 1024 {
        return Err("Handshake message too large".into());
    }
    let mut buf = vec![0; len];
    stream.read_exact(&mut buf).await?;
    Ok((message_type, buf))
}
pub fn chat_type_to_str(chat_type: i32) -> Result<String, Box<dyn Error>> {
    let chat_type = ChatType::try_from_primitive(chat_type as u32)?;
    Ok(match chat_type {
        ChatType::Interactive => "interactive",
        ChatType::NonInteractive => "noninteractive",
    }
    .to_string())
}
pub async fn handle_heartbeat(msg: HeartbeatRequest, stream: &mut TlsStream<TcpStream>) -> Result<(), Box<dyn Error>> {
    let response = HeartbeatResponse { random: msg.random };
    write_message(MessageType::HeartbeatResponse, response, stream).await?;
    Ok(())
}
pub async fn send_heartbeat(stream: &mut TlsStream<TcpStream>) -> Result<(), Box<dyn Error>> {
    let heartbeat = HeartbeatRequest { random: rand::random() };
    write_message(MessageType::HeartbeatRequest, heartbeat, stream).await?;
    let (_, buf) = read_message(stream, Some(MessageType::HeartbeatResponse)).await?;
    let response = HeartbeatResponse::decode(&*buf)?;
    if response.random != heartbeat.random {
        return Err("Invalid heartbeat response".into());
    }
    Ok(())
}
