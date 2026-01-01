#![forbid(unsafe_code)]

use num_enum::TryFromPrimitive;
use prost::Message;
use std::{env, error::Error};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

include!(concat!(env!("OUT_DIR"), "/agent.rs"));

pub const MAGIC: usize = 13609285118649036194;
pub const API_VERSION: usize = 1;

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
}
#[repr(u32)]
pub enum CommandOrigin {
    SystemIdentification = 0,
    UserRequested = 1,
    LLMTool = 2,
}
#[derive(PartialEq, TryFromPrimitive)]
#[repr(u32)]
pub enum ChatType {
    Interactive = 0,
    NonInteractive = 1,
}

pub fn print_startup(component: &str, version: &str) {
    println!("--------------------------------------------------");
    println!("Starting Atlas {} {}", component, version);
    #[cfg(debug_assertions)]
    println!("Running in debug mode, this is INSECURE");
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
