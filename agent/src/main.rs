use crate::install::install_agent;
use crate::messages::core::handle_command;
use crate::messages::ssh::handle_ssh_agent;
use crate::messages::update::handle_update;
use crate::misc::start_connection;
use crate::supervise::supervise_agent;
use crate::{models::Setting, schema::settings};
use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::prelude::*;
use diesel::{Connection, SqliteConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use mlua::Lua;
use prost::Message;
use shared_api::{CommandRequest, HandshakePurpose, HeartbeatRequest, MessageType, StartSshAgentRequest, UpdateRequest, handle_heartbeat, print_startup, read_message};
use std::env::args;
use std::ffi::CStr;
use std::io::stdin;
use std::process::{Stdio, exit};
use std::ptr;
use std::{env, error::Error, io::Read, sync::Arc, time::Duration};
use tokio::fs::DirBuilder;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::sleep;
use tokio_rustls::rustls::pki_types::CertificateDer;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::{TlsConnector, rustls};

mod install;
mod messages;
mod misc;
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
        if cmd == "execute-lua" {
            let mut code = String::new();
            stdin().read_to_string(&mut code).unwrap();
            let lua = Lua::new();
            lua.load(code).exec().unwrap();
            return;
        } else if cmd == "execute-python" {
            let mut code = String::new();
            stdin().read_to_string(&mut code).unwrap();
            let mut cmd = Command::new("/usr/bin/env").arg("python3").arg("-").stdin(Stdio::piped()).spawn().unwrap();
            cmd.stdin.take().unwrap().write_all(code.as_bytes()).await.unwrap();
            let result = cmd.wait().await.unwrap();
            exit(result.code().unwrap());
        } else if cmd == "self-test" {
            println!("OK");
            return;
        } else if cmd == "install" {
            install_agent().await.unwrap();
            return;
        } else if cmd == "supervise" {
            supervise_agent().await.unwrap();
            return;
        } else if cmd == "pubkey" {
            let mut db = SqliteConnection::establish(DATABASE_URL).unwrap();
            let keypair = settings::table.find("keypair").first::<Setting>(&mut db).unwrap().value;
            let keypair = BASE64_STANDARD.decode(keypair).unwrap();
            let keypair = Ed25519KeyPair::from_pkcs8(&keypair).unwrap();
            println!("{}", BASE64_STANDARD.encode(keypair.public_key()));
            return;
        } else {
            eprintln!("Unknown command: {}", cmd);
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
            eprintln!("Error in connection: {}", e);
            sleep(Duration::from_secs(10)).await;
        } else {
            sleep(Duration::from_secs(2)).await;
        }
    }
}
async fn open_connection(host: &str, tls_connector: TlsConnector, db: &mut SqliteConnection) -> Result<(), Box<dyn Error>> {
    let mut stream = match start_connection(host, &tls_connector, db, HandshakePurpose::General).await? {
        Some(s) => s,
        None => {
            return Ok(());
        }
    };
    println!("Connected to server");
    loop {
        let (message_type, buf) = read_message(&mut stream, None).await?;
        match message_type {
            MessageType::HeartbeatRequest => handle_heartbeat(HeartbeatRequest::decode(&*buf)?, &mut stream).await,
            MessageType::CommandRequest => handle_command(CommandRequest::decode(&*buf)?, &mut stream).await,
            MessageType::UpdateRequest => handle_update(UpdateRequest::decode(&*buf)?, &mut stream).await,
            MessageType::StartSSHAgentRequest => handle_ssh_agent(StartSshAgentRequest::decode(&*buf)?, &mut stream, host.to_string(), tls_connector.clone()).await,
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
