#![forbid(unsafe_code)]

use crate::device::process_devices;
use crate::llm::process_chats;
use crate::timers::process_timers;
use crate::tool::process_tool_calls;
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use argon2::{Algorithm, Argon2, Params, PasswordHasher, Version};
use aws_lc_rs::encoding::AsDer;
use aws_lc_rs::rsa::{KeySize, PrivateDecryptingKey};
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::{PgConnection, QueryDsl, RunQueryDsl};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use fred::prelude::{EventInterface, PubsubInterface};
use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P521_SHA512};
use shared_api::{CommandOrigin, print_startup};
use shared_server::models::{Setting, User};
use shared_server::schema::{commands, settings, users};
use shared_server::{get_db_pool, get_sub_client, listen_for_restarts, notify_server};
use std::error::Error;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::time::{sleep, timeout};

mod device;
mod llm;
mod lua;
mod timers;
mod tool;
mod tools;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("../shared-server/migrations");
static DB_POOL: OnceLock<Pool<ConnectionManager<PgConnection>>> = OnceLock::new();

#[tokio::main]
async fn main() {
    print_startup("master", env!("CARGO_PKG_VERSION"));
    dotenvy::dotenv().ok();
    notify_server("MASTER:RESTART").await.unwrap();
    listen_for_restarts("MASTER".to_string()).await;
    let pool = get_db_pool().unwrap();
    {
        let mut db = pool.get().unwrap();
        db.run_pending_migrations(MIGRATIONS).unwrap();
        create_settings(&mut db);
        diesel::delete(commands::table.filter(commands::origin.eq(CommandOrigin::LLMTool as i32))).execute(&mut db).unwrap();
    }
    let device_pool = pool.clone();
    let llm_pool = pool.clone();
    let tool_pool = pool.clone();
    let timers_pool = pool.clone();
    DB_POOL.set(pool).unwrap();
    tokio::spawn(async { tasks_loop_wrapper(device_pool, "MASTER:DEVICES", process_devices).await });
    tokio::spawn(async { tasks_loop_wrapper(llm_pool, "MASTER:LLM", process_chats).await });
    tokio::spawn(async { tasks_loop_wrapper(tool_pool, "MASTER:TOOLS", process_tool_calls).await });
    tokio::spawn(async { tasks_loop_wrapper(timers_pool, "MASTER:TIMERS", process_timers).await });
    loop {
        sleep(Duration::from_hours(24)).await;
    }
}
async fn tasks_loop_wrapper<T: Future<Output = Result<(), Box<dyn Error>>>>(pool: Pool<ConnectionManager<PgConnection>>, sub_channel: &str, func: fn(Pool<ConnectionManager<PgConnection>>) -> T) {
    loop {
        tasks_loop(pool.clone(), sub_channel, func).await.ok();
    }
}
async fn tasks_loop<T: Future<Output = Result<(), Box<dyn Error>>>>(
    pool: Pool<ConnectionManager<PgConnection>>,
    sub_channel: &str,
    func: fn(Pool<ConnectionManager<PgConnection>>) -> T,
) -> Result<(), Box<dyn Error>> {
    let sub_client = get_sub_client().await?;
    sub_client.subscribe(sub_channel).await?;
    let mut receiver = sub_client.message_rx();
    loop {
        if let Err(e) = func(pool.clone()).await {
            println!("Error while running tasks: {}", e)
        }
        timeout(Duration::from_secs(60), async { receiver.recv().await.ok() }).await.ok();
    }
}
fn create_settings(db: &mut PgConnection) {
    db.build_transaction()
        .serializable()
        .run(|db| {
            let setting_exists = settings::table.find("allow_new_devices").first::<Setting>(db).optional()?.is_some();
            if setting_exists {
                return Ok(());
            }
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "allow_new_devices".to_owned(),
                    value: "true".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "apiserver_host".to_owned(),
                    value: "localhost".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "openrouter_model".to_owned(),
                    value: "z-ai/glm-5.1".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "openrouter_key".to_owned(),
                    value: "".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "mail_server".to_owned(),
                    value: "".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "mail_username".to_owned(),
                    value: "".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "mail_password".to_owned(),
                    value: "".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "mail_from".to_owned(),
                    value: "".to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "mail_to".to_owned(),
                    value: "".to_string(),
                })
                .execute(db)?;
            Ok::<(), Box<dyn Error>>(())
        })
        .unwrap();
    db.build_transaction()
        .serializable()
        .run(|db| {
            let key_exists = settings::table.find("tls_key").first::<Setting>(db).optional()?.is_some();
            if key_exists {
                return Ok(());
            }
            println!("TLS key not found, generating...");
            let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P521_SHA512)?;
            let mut cert_params = CertificateParams::new(vec!["apiserver".to_owned()])?;
            cert_params.key_identifier_method = rcgen::KeyIdMethod::Sha512;
            let cert = cert_params.self_signed(&key_pair)?;
            let cert_pem = cert.pem().trim().to_string();
            println!("{}", cert_pem);

            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "tls_key".to_owned(),
                    value: key_pair.serialize_pem().trim().to_string(),
                })
                .execute(db)?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "tls_cert".to_owned(),
                    value: cert_pem,
                })
                .execute(db)?;

            let jwt_secret: [u8; 64] = rand::random();
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "jwt_secret".to_owned(),
                    value: BASE64_STANDARD.encode(jwt_secret),
                })
                .execute(db)?;
            let registration_token: [u8; 64] = rand::random();
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "registration_token".to_owned(),
                    value: BASE64_STANDARD.encode(registration_token),
                })
                .execute(db)?;
            let web_encryption_key = PrivateDecryptingKey::generate(KeySize::Rsa8192)?.as_der()?;
            diesel::insert_into(settings::table)
                .values(Setting {
                    name: "web_encryption_key".to_owned(),
                    value: BASE64_STANDARD.encode(web_encryption_key.as_ref()),
                })
                .execute(db)?;
            Ok::<(), Box<dyn Error>>(())
        })
        .unwrap();
    db.build_transaction()
        .serializable()
        .run(|db| {
            let users_count: i64 = users::table.count().get_result(db)?;
            if users_count != 0 {
                return Ok(());
            }
            println!("No users found, creating admin account...");
            let salt = SaltString::generate(OsRng);
            let password: [u8; 64] = rand::random();
            let password = BASE64_STANDARD.encode(password);
            let params = Params::new(128 * 1024, 1, 1, Some(64)).or(Err("Invalid argon2 parameters"))?;
            let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            let hash = argon.hash_password(password.as_bytes(), &salt).or(Err("Couldn't hash password"))?;
            diesel::insert_into(users::table)
                .values(User {
                    id: None,
                    username: "admin".to_owned(),
                    password: hash.to_string(),
                    admin: true,
                    jwt_counter: 0,
                })
                .execute(db)?;
            println!("Initial Credentials: admin / {}", password);
            Ok::<(), Box<dyn Error>>(())
        })
        .unwrap();
}
