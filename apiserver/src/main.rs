#![forbid(unsafe_code)]

use ::aws_lc_rs::{
    constant_time::verify_slices_are_equal,
    signature::{ED25519, UnparsedPublicKey},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::{
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use prost::Message;
use rustls::{
    ServerConfig,
    crypto::{
        CryptoProvider,
        aws_lc_rs::{self, cipher_suite::TLS13_AES_256_GCM_SHA384},
    },
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
};
use sha2::{Digest, Sha512};
use shared_api::{
    API_VERSION, CommandOrigin, CommandRequest, CommandResponse, ContainerOrigin, HandshakeFinish, HandshakePurpose, HandshakeRequest, HandshakeResponse, HeartbeatRequest, MAGIC, MessageType,
    SUPPORTED_ARCH, SUPPORTED_OS, StartSshAgentRequest, UpdateRequest, UpdateResponse, handle_heartbeat, print_startup, read_message, send_heartbeat, write_message,
};
use shared_server::{
    binary::personalize_binary,
    get_db_pool, get_valkey_client, listen_for_restarts,
    models::{Command, Container, DevelopmentEnvironment, Device, Setting},
    notify_server,
    schema::{commands, containers, development_environments, devices, settings},
    wait_for_pubsub_msg,
};
use ssh_key::PrivateKey;
use std::{
    error::Error,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{self, crypto::aws_lc_rs::kx_group::X25519MLKEM768},
};
use uuid::Uuid;

use crate::ssh::handle_ssh_agent;

mod ssh;

#[tokio::main]
async fn main() {
    print_startup("apiserver", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    listen_for_restarts("API".to_string()).await;
    CryptoProvider {
        cipher_suites: vec![TLS13_AES_256_GCM_SHA384],
        kx_groups: vec![X25519MLKEM768],
        ..aws_lc_rs::default_provider()
    }
    .install_default()
    .unwrap();
    let pool = get_db_pool().unwrap();
    let tls_acceptor;
    {
        let mut db = pool.get().unwrap();
        let tls_key: Setting = settings::table.find("tls_key").first(&mut db).unwrap();
        let tls_cert: Setting = settings::table.find("tls_cert").first(&mut db).unwrap();
        let key_der = PrivateKeyDer::from_pem_slice(tls_key.value.as_bytes()).unwrap();
        let cert_der = CertificateDer::from_pem_slice(tls_cert.value.as_bytes()).unwrap();
        let tls_config = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();
        tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));
    }
    let listener = TcpListener::bind("0.0.0.0:8612").await.unwrap();
    loop {
        let result = accept_connection(&listener, tls_acceptor.clone(), pool.clone()).await;
        if let Err(e) = result {
            eprintln!("Error in connection: {}", e)
        }
    }
}

async fn accept_connection(listener: &TcpListener, tls_acceptor: TlsAcceptor, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let (stream, addr) = listener.accept().await?;
    let ip = addr.ip().to_string();
    tokio::spawn(async {
        if let Err(e) = handle_connection(stream, ip, tls_acceptor, pool).await {
            eprintln!("Error in connection: {}", e);
        }
    });
    Ok(())
}

async fn handle_connection(stream: TcpStream, ip: String, tls_acceptor: TlsAcceptor, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut stream = tls_acceptor.accept(stream).await?.into();
    let (_, buf) = read_message(&mut stream, Some(MessageType::HandshakeRequest)).await?;
    let handshake_request = HandshakeRequest::decode(&*buf)?;
    if handshake_request.magic != MAGIC as u64 || handshake_request.api_version != API_VERSION as u64 {
        return Err("Invalid client version".into());
    }
    if !handshake_request.hostname.is_ascii() || !handshake_request.hostname.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Invalid hostname".into());
    }
    if !SUPPORTED_ARCH.contains(&handshake_request.arch.as_str()) {
        return Err("Invalid architecture".into());
    }
    if !SUPPORTED_OS.contains(&handshake_request.os.as_str()) {
        return Err("Invalid OS".into());
    }
    let mut handshake_response = HandshakeResponse {
        magic: MAGIC as u64,
        api_version: API_VERSION as u64,
        device_id: "".to_owned(),
        auth_challenge: vec![],
        reset: false,
    };
    let mut db = pool.get()?;
    if handshake_request.register_device {
        let allowed: Setting = settings::table.find("allow_new_devices").first(&mut db)?;
        if allowed.value != "true" {
            return Err("No new devices are allowed".into());
        }
        let registration_token: Setting = settings::table.find("registration_token").first(&mut db)?;
        let registration_token = BASE64_STANDARD.decode(registration_token.value)?;
        if verify_slices_are_equal(&registration_token, &handshake_request.registration_token).is_err() {
            #[cfg(not(debug_assertions))]
            return Err("Incorrect registration token".into());
        }
        let device: Device = diesel::insert_into(devices::table)
            .values(Device {
                id: None,
                arch: handshake_request.arch,
                os: handshake_request.os,
                version: handshake_request.version,
                pubkey: handshake_request.pubkey,
                approved: false,
                hostname: handshake_request.hostname,
                first_connection: SystemTime::now(),
                last_connection: SystemTime::now(),
                ip,
                agent_hash: handshake_request.agent_hash,
                container_id: None,
            })
            .get_result(&mut db)?;
        handshake_response.device_id = device.id.ok_or("Invalid database result")?.to_string();
        write_message(MessageType::HandshakeResponse, handshake_response, &mut stream).await?;
        notify_server("WEB:REFRESH").await?;
        return Ok(());
    }
    let uuid = Uuid::from_str(&handshake_request.device_id)?;
    let device_exists = devices::table.find(uuid).first::<Device>(&mut db).optional()?.is_some();
    if !device_exists {
        handshake_response.reset = true;
        write_message(MessageType::HandshakeResponse, handshake_response, &mut stream).await?;
        return Ok(());
    }
    let mut device: Device = devices::table.find(uuid).first(&mut db)?;
    if device.pubkey != handshake_request.pubkey {
        return Err("Invalid pubkey".into());
    }
    if !device.approved {
        return Err("Device isn't approved".into());
    }
    let challenge: [u8; 128] = rand::random();
    handshake_response.auth_challenge = challenge.to_vec();
    write_message(MessageType::HandshakeResponse, handshake_response, &mut stream).await?;
    drop(db);
    let (_, buf) = read_message(&mut stream, Some(MessageType::HandshakeFinish)).await?;
    let mut db = pool.get()?;
    let handshake_finish = HandshakeFinish::decode(&*buf)?;
    let public_key = UnparsedPublicKey::new(&ED25519, &device.pubkey);
    public_key.verify(&challenge, &handshake_finish.auth_response)?;

    device.arch = handshake_request.arch;
    device.os = handshake_request.os;
    device.version = handshake_request.version;
    device.last_connection = SystemTime::now();
    device.ip = ip;
    device.agent_hash = handshake_request.agent_hash;
    diesel::update(devices::table.find(uuid)).set(&device).execute(&mut db)?;
    notify_server("WEB:REFRESH").await?;

    if handshake_request.purpose == HandshakePurpose::SSHAgent as u32 {
        handle_ssh_agent(stream, db, uuid).await?;
        return Ok(());
    }
    if handshake_request.purpose != HandshakePurpose::General as u32 {
        return Err("Invalid handshake purpose".into());
    }

    if let Ok(agent) = personalize_binary(&mut db, &device.arch, &device.os).await.map_err(|_| "Failed to get agent") {
        let hash = Sha512::digest(&agent).to_vec();
        if hash != device.agent_hash {
            let request = UpdateRequest { agent, hash };
            write_message(MessageType::UpdateRequest, request, &mut stream).await?;
            let (_, buf) = read_message(&mut stream, Some(MessageType::UpdateResponse)).await?;
            let response = UpdateResponse::decode(&*buf)?;
            if response.success {
                return Ok(());
            }
        }
    }

    if let Some(container_id) = device.container_id {
        let container: Container = containers::table.find(container_id).first(&mut db)?;
        if container.origin == ContainerOrigin::DevelopmentEnvironment as i32 {
            let development_environment: DevelopmentEnvironment = development_environments::table.find(container_id).first(&mut db)?;
            let private_key = PrivateKey::from_openssh(development_environment.ssh_key)?;
            write_message(
                MessageType::StartSSHAgentRequest,
                StartSshAgentRequest {
                    key: private_key.public_key().to_openssh()?,
                },
                &mut stream,
            )
            .await?;
            read_message(&mut stream, Some(MessageType::StartSSHAgentResponse)).await?;
        }
    }

    let (mut pubsub_client, mut receiver) = get_valkey_client().await?;
    pubsub_client.subscribe(format!("API:DEVICE:{}", uuid)).await?;
    loop {
        let device_exists = devices::table.find(uuid).first::<Device>(&mut db).optional()?.is_some();
        if !device_exists {
            return Err("Device no longer exists".into());
        }
        let command: Option<Command> = commands::table.filter(commands::device.eq(uuid).and(commands::completed.eq(false))).first(&mut db).optional()?;
        if let Some(mut command) = command {
            if command.initiated {
                command.completed = true;
                command.status_code = 100;
                command.stdout = "".to_string();
                command.stderr = "Command didn't complete".to_string();
                command.completed_timestamp = Some(SystemTime::now());
                diesel::update(commands::table.find(command.id.ok_or("Invalid command")?)).set(&command).execute(&mut db)?;
            } else {
                command.initiated = true;
                diesel::update(commands::table.find(command.id.ok_or("Invalid command")?)).set(&command).execute(&mut db)?;
                let request = CommandRequest {
                    code: command.code.clone(),
                    language: command.language as u32,
                };
                write_message(MessageType::CommandRequest, request, &mut stream).await?;
                let (mut message_type, mut buf);
                loop {
                    (message_type, buf) = read_message(&mut stream, None).await?;
                    if message_type == MessageType::HeartbeatRequest {
                        let request = HeartbeatRequest::decode(&*buf)?;
                        handle_heartbeat(request, &mut stream).await?;
                    } else if message_type == MessageType::CommandResponse {
                        break;
                    } else {
                        return Err("Invalid message type".into());
                    }
                }
                let response = CommandResponse::decode(&*buf)?;
                command.completed = true;
                command.status_code = response.status as i32;
                command.stdout = response.stdout.replace("\0", "");
                command.stderr = response.stderr.replace("\0", "");
                command.completed_timestamp = Some(SystemTime::now());
                diesel::update(commands::table.find(command.id.ok_or("Invalid command")?)).set(&command).execute(&mut db)?;
                diesel::update(devices::table.find(uuid)).set(devices::last_connection.eq(SystemTime::now())).execute(&mut db)?;
            }
            notify_server("WEB:REFRESH").await?;
            if command.origin == CommandOrigin::SystemIdentification as i32 {
                notify_server("MASTER:DEVICES").await?;
            } else if command.origin == CommandOrigin::LLMTool as i32 {
                notify_server(&format!("MASTER:CMD:{}", command.id.ok_or("Invalid command")?)).await?;
            }
        } else if timeout(Duration::from_secs(60), async {
            wait_for_pubsub_msg(&mut receiver).await;
        })
        .await
        .is_err()
        {
            send_heartbeat(&mut stream).await?;
            diesel::update(devices::table.find(uuid)).set(devices::last_connection.eq(SystemTime::now())).execute(&mut db)?;
        }
    }
}
