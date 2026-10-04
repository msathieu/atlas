use aws_lc_rs::encoding::AsDer;
use aws_lc_rs::rsa::{KeySize, PrivateDecryptingKey};
use aws_lc_rs::signature::Ed25519KeyPair;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::OptionalExtension;
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P521_SHA512};
use shared_server::hash_password;
use shared_server::models::User;
use shared_server::schema::users;
use shared_server::{models::Setting, schema::settings};
use std::error::Error;
use webauthn_rs::fake::{FakePasskeyDistribution, WebauthnFakeCredentialGenerator};

async fn does_setting_exist(db: &mut AsyncPgConnection, name: &str) -> Result<bool, Box<dyn Error>> {
    let setting_exists = settings::table.find(name).first::<Setting>(db).await.optional()?.is_some();
    Ok(setting_exists)
}
pub async fn create_settings(db: &mut AsyncPgConnection) -> Result<(), Box<dyn Error>> {
    if !does_setting_exist(db, "database_migration").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "database_migration".to_owned(),
                value: "1".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "allow_new_devices").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "allow_new_devices".to_owned(),
                value: "true".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_default_provider").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_default_provider".to_owned(),
                value: "generic".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_generic_endpoint").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_generic_endpoint".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_generic_key").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_generic_key".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_generic_model").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_generic_model".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_openrouter_key").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_openrouter_key".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "llm_openrouter_model").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "llm_openrouter_model".to_owned(),
                value: "z-ai/glm-5.3-flash".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "mail_username").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "mail_username".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "mail_password").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "mail_password".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "mail_from").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "mail_from".to_owned(),
                value: "".to_string(),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "jwt_secret").await? {
        let jwt_secret: [u8; 64] = rand::random();
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "jwt_secret".to_owned(),
                value: BASE64_STANDARD.encode(jwt_secret),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "registration_token").await? {
        let registration_token: [u8; 64] = rand::random();
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "registration_token".to_owned(),
                value: BASE64_STANDARD.encode(registration_token),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "web_encryption_key").await? {
        let web_encryption_key = PrivateDecryptingKey::generate(KeySize::Rsa8192)?.as_der()?;
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "web_encryption_key".to_owned(),
                value: BASE64_STANDARD.encode(web_encryption_key.as_ref()),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "web_signing_key").await? {
        let web_signing_key = Ed25519KeyPair::generate()?.to_pkcs8()?;
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "web_signing_key".to_owned(),
                value: BASE64_STANDARD.encode(web_signing_key.as_ref()),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "fake_passkey_key").await? {
        let fake_passkey_key = WebauthnFakeCredentialGenerator::<FakePasskeyDistribution>::new_hmac_key()?;
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "fake_passkey_key".to_owned(),
                value: BASE64_STANDARD.encode(fake_passkey_key),
            })
            .execute(db)
            .await?;
    }
    if !does_setting_exist(db, "apk_hash").await? {
        diesel::insert_into(settings::table)
            .values(Setting {
                name: "apk_hash".to_owned(),
                value: "".to_owned(),
            })
            .execute(db)
            .await?;
    }
    db.build_transaction()
        .serializable()
        .run(async |db| {
            if !does_setting_exist(db, "tls_key").await? {
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
                    .execute(db)
                    .await?;
                diesel::insert_into(settings::table)
                    .values(Setting {
                        name: "tls_cert".to_owned(),
                        value: cert_pem,
                    })
                    .execute(db)
                    .await?;
            }
            Ok::<(), Box<dyn Error>>(())
        })
        .await?;
    db.build_transaction()
        .serializable()
        .run(async |db| {
            let users_count: i64 = users::table.count().get_result(db).await?;
            if users_count != 0 {
                return Ok(());
            }
            println!("No users found, creating admin account...");
            let password: [u8; 64] = rand::random();
            let password = BASE64_STANDARD.encode(password);
            let hash = hash_password(password.clone()).await?;
            diesel::insert_into(users::table)
                .values(User {
                    id: None,
                    username: "admin".to_owned(),
                    password: hash,
                    admin: true,
                    jwt_counter: 0,
                    enabled: true,
                    password_enabled: true,
                    last_login: None,
                    email: None,
                })
                .execute(db)
                .await?;
            println!("Initial Credentials: admin / {}", password);
            Ok::<(), Box<dyn Error>>(())
        })
        .await?;
    Ok(())
}
