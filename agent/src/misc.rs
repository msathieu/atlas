use crate::REGISTRATION_TOKEN;
use crate::models::Setting;
use crate::schema::settings;
use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::SqliteConnection;
use diesel::delete;
use diesel::prelude::*;
use gethostname::gethostname;
use prost::Message;
use sha2::{Digest, Sha512};
use shared_api::HandshakeFinish;
use shared_api::HandshakeResponse;
use shared_api::MessageType;
use shared_api::read_message;
use shared_api::write_message;
use shared_api::{API_VERSION, HandshakePurpose, HandshakeRequest, MAGIC};
use std::env::current_exe;
use std::process::exit;
use std::ptr;
use std::{env, error::Error};
use tokio::fs;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;

pub async fn start_connection(host: &str, tls_connector: &TlsConnector, db: &mut SqliteConnection, purpose: HandshakePurpose) -> Result<Option<TlsStream<TcpStream>>, Box<dyn Error>> {
    let stream = TcpStream::connect(format!("{}:8612", host)).await?;
    let server_name = ServerName::DnsName("apiserver".try_into()?);
    let mut stream = tls_connector.connect(server_name, stream).await?.into();
    let keypair = settings::table.find("keypair").first::<Setting>(db)?.value;
    let keypair = BASE64_STANDARD.decode(keypair)?;
    let keypair = Ed25519KeyPair::from_pkcs8(&keypair)?;
    let hash;
    {
        let file = fs::read(current_exe()?).await?;
        hash = Sha512::digest(file).to_vec();
    }
    let registration_token = unsafe { ptr::read_volatile(&REGISTRATION_TOKEN) };
    let mut handshake_request = HandshakeRequest {
        magic: MAGIC as u64,
        api_version: API_VERSION as u64,
        arch: env::consts::ARCH.to_owned(),
        os: env::consts::OS.to_owned(),
        device_id: "".to_owned(),
        register_device: false,
        pubkey: keypair.public_key().as_ref().to_owned(),
        hostname: gethostname().into_string().or(Err("Invalid hostname"))?,
        version: env!("CARGO_PKG_VERSION").to_string(),
        agent_hash: hash,
        registration_token: vec![],
        purpose: purpose as u32,
    };
    let device_id = settings::table.find("device_id").first::<Setting>(db).optional()?;
    match &device_id {
        Some(id) => handshake_request.device_id = id.value.to_owned(),
        None => {
            handshake_request.register_device = true;
            handshake_request.registration_token = registration_token.to_vec();
        }
    }
    write_message(MessageType::HandshakeRequest, handshake_request, &mut stream).await?;
    let (_, buf) = read_message(&mut stream, Some(MessageType::HandshakeResponse)).await?;
    let handshake_response = HandshakeResponse::decode(&*buf)?;
    if handshake_response.magic != MAGIC as u64 || handshake_response.api_version != API_VERSION as u64 {
        return Err("Invalid server version".into());
    }
    if handshake_response.reset {
        delete(settings::table).execute(db)?;
        exit(0);
    }
    if device_id.is_none() {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "device_id".to_owned(),
                value: handshake_response.device_id,
            })
            .execute(db)?;
        return Ok(None);
    }
    let signature = keypair.sign(&handshake_response.auth_challenge);
    let handshake_finish = HandshakeFinish {
        auth_response: signature.as_ref().to_vec(),
    };
    write_message(MessageType::HandshakeFinish, handshake_finish, &mut stream).await?;
    Ok(Some(stream))
}
