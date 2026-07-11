use crate::install::install_agent;
use crate::messages::core::handle_command;
use crate::messages::update::handle_update;
use crate::supervise::supervise_agent;
use crate::{messages::core::handle_heartbeat, models::Setting, schema::settings};
use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::{Connection, SqliteConnection};
use diesel::{delete, prelude::*};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use gethostname::gethostname;
use mlua::Lua;
use prost::Message;
use sha2::{Digest, Sha512};
use shared_api::{API_VERSION, CommandRequest, HandshakeFinish, HandshakeRequest, HandshakeResponse, HeartbeatRequest, MAGIC, MessageType, UpdateRequest, print_startup, read_message, write_message};
use std::env::{args, current_exe};
use std::ffi::CStr;
use std::io::stdin;
use std::process::exit;
use std::ptr;
use std::{env, error::Error, io::Read, sync::Arc, time::Duration};
use tokio::fs::{self, DirBuilder};
use tokio::net::TcpStream;
use tokio::time::sleep;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::{TlsConnector, rustls};

mod install;
mod messages;
mod models;
mod schema;
mod supervise;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[unsafe(link_section = "apiserver_host")]
static HOST: [u8; 256] = [0; 256];
#[unsafe(link_section = "apiserver_cert")]
static TLS_CERT: [u8; 1024] = [0; 1024];
#[unsafe(link_section = "registration_token")]
static REGISTRATION_TOKEN: [u8; 64] = [0; 64];

#[cfg(debug_assertions)]
const DATABASE_URL: &str = "agent.db";
#[cfg(not(debug_assertions))]
const DATABASE_URL: &str = "/var/lib/atlas/agent.db";

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    {
        dotenvy::dotenv().ok();
    }
    let args: Vec<String> = args().collect();
    if let Some(cmd) = args.get(1) {
        if cmd == "execute-cmd" {
            let mut code = String::new();
            stdin().read_to_string(&mut code).unwrap();
            let lua = Lua::new();
            lua.load(code).exec().unwrap();
            return;
        } else if cmd == "self-test" {
            println!("OK");
            return;
        } else if cmd == "install" {
            install_agent().await.unwrap();
            return;
        } else if cmd == "supervise" {
            supervise_agent().await.unwrap();
            return;
        } else {
            println!("Unknown command: {}", cmd);
            exit(1);
        }
    }
    print_startup("agent", env!("CARGO_PKG_VERSION"));
    let mut dir_builder = DirBuilder::new();
    dir_builder.mode(0o700).recursive(true);
    #[cfg(not(debug_assertions))]
    {
        dir_builder.create("/var/lib/atlas").await.unwrap();
    }
    let mut db = SqliteConnection::establish(DATABASE_URL).unwrap();
    db.run_pending_migrations(MIGRATIONS).unwrap();
    create_keypair(&mut db);
    let mut cert_store = RootCertStore::empty();
    let _host = unsafe { ptr::read_volatile(&HOST) };
    let _tls_cert = unsafe { ptr::read_volatile(&TLS_CERT) };
    let mut _host = CStr::from_bytes_until_nul(&_host).unwrap().to_str().unwrap().to_string();
    let mut _tls_cert = CStr::from_bytes_until_nul(&_tls_cert).unwrap().to_str().unwrap().to_string();
    #[cfg(debug_assertions)]
    {
        _host = "localhost".to_string();
        _tls_cert = env::var("APISERVER_CERT").unwrap();
    }
    println!("Connecting to {}", _host);
    cert_store.add(CertificateDer::from_pem_reader(_tls_cert.trim().as_bytes()).unwrap()).unwrap();
    let tls_config = ClientConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_root_certificates(cert_store)
        .with_no_client_auth();
    let tls_config = Arc::new(tls_config);
    let tls_connector = TlsConnector::from(tls_config);
    loop {
        let result = open_connection(&_host, tls_connector.clone(), &mut db).await;
        if let Err(e) = result {
            println!("Error in connection: {}", e)
        }
        sleep(Duration::from_secs(10)).await;
    }
}
async fn open_connection(host: &str, tls_connector: TlsConnector, db: &mut SqliteConnection) -> Result<(), Box<dyn Error>> {
    let stream = TcpStream::connect(format!("{}:8612", host)).await?;
    let server_name = ServerName::DnsName("apiserver".try_into()?);
    let mut stream = tls_connector.connect(server_name, stream).await?;
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
        return Ok(());
    }
    let signature = keypair.sign(&handshake_response.auth_challenge);
    let handshake_finish = HandshakeFinish {
        auth_response: signature.as_ref().to_vec(),
    };
    write_message(MessageType::HandshakeFinish, handshake_finish, &mut stream).await?;
    println!("Connected to server");
    loop {
        let (message_type, buf) = read_message(&mut stream, None).await?;
        match message_type {
            MessageType::HeartbeatRequest => handle_heartbeat(HeartbeatRequest::decode(&*buf)?, &mut stream).await,
            MessageType::CommandRequest => handle_command(CommandRequest::decode(&*buf)?, &mut stream).await,
            MessageType::UpdateRequest => handle_update(UpdateRequest::decode(&*buf)?, &mut stream).await,
            _ => return Err("Invalid message type".into()),
        }?
    }
}
fn create_keypair(db: &mut SqliteConnection) {
    let key_exists = settings::table.find("keypair").first::<Setting>(db).optional().unwrap().is_some();
    if key_exists {
        return;
    }
    println!("Keypair not found, generating...");
    let key_pair = Ed25519KeyPair::generate().unwrap();
    diesel::insert_into(settings::table)
        .values(Setting {
            name: "keypair".to_owned(),
            value: BASE64_STANDARD.encode(key_pair.to_pkcs8().unwrap()),
        })
        .execute(db)
        .unwrap();
    println!("Public Key: {}", BASE64_STANDARD.encode(key_pair.public_key()));
}
