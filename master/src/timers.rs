use aws_lc_rs::{
    encoding::AsDer,
    rsa::{KeySize, PrivateDecryptingKey},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Pool};
use shared_api::ChatType;
use shared_server::models::{Chat, Setting};
use shared_server::schema::{chats, settings};
use std::error::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::task::spawn_blocking;

const JWT_ROTATION_INTERVAL: Duration = Duration::from_hours(7 * 24);
const ENCRYPTION_ROTATION_INTERVAL: Duration = Duration::from_hours(7 * 24);

async fn rotate_encryption_key(db: &mut AsyncPgConnection) -> Result<(), Box<dyn Error>> {
    let new_key = spawn_blocking(|| PrivateDecryptingKey::generate(KeySize::Rsa8192)?.as_der()).await??;
    let new_key = BASE64_STANDARD.encode(new_key.as_ref());
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    db.build_transaction()
        .serializable()
        .run(async |db| {
            let rotated_at: Setting = settings::table.find("web_encryption_key_rotated_at").first(db).await?;
            let rotated_at: u64 = rotated_at
                .value
                .parse()
                .map_err(|_| diesel::result::Error::DeserializationError("Failed to deserialize rotated_at".into()))?;
            if rotated_at + ENCRYPTION_ROTATION_INTERVAL.as_secs() >= now {
                return Ok::<(), diesel::result::Error>(());
            }
            let current: Setting = settings::table.find("web_encryption_key").first(db).await?;
            diesel::update(settings::table.find("web_encryption_key_old"))
                .set(settings::value.eq(current.value))
                .execute(db)
                .await?;
            diesel::update(settings::table.find("web_encryption_key")).set(settings::value.eq(new_key)).execute(db).await?;
            diesel::update(settings::table.find("web_encryption_key_rotated_at"))
                .set(settings::value.eq(now.to_string()))
                .execute(db)
                .await?;
            Ok(())
        })
        .await?;
    Ok(())
}
async fn rotate_jwt_secret(db: &mut AsyncPgConnection) -> Result<(), Box<dyn Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    db.build_transaction()
        .serializable()
        .run(async |db| {
            let rotated_at: Setting = settings::table.find("jwt_secret_rotated_at").first(db).await?;
            let rotated_at: u64 = rotated_at
                .value
                .parse()
                .map_err(|_| diesel::result::Error::DeserializationError("Failed to deserialize rotated_at".into()))?;
            if rotated_at + JWT_ROTATION_INTERVAL.as_secs() >= now {
                return Ok::<(), diesel::result::Error>(());
            }
            let current: Setting = settings::table.find("jwt_secret").first(db).await?;
            let new_secret: [u8; 64] = rand::random();
            diesel::update(settings::table.find("jwt_secret_old")).set(settings::value.eq(current.value)).execute(db).await?;
            diesel::update(settings::table.find("jwt_secret"))
                .set(settings::value.eq(BASE64_STANDARD.encode(new_secret)))
                .execute(db)
                .await?;
            diesel::update(settings::table.find("jwt_secret_rotated_at"))
                .set(settings::value.eq(now.to_string()))
                .execute(db)
                .await?;
            Ok(())
        })
        .await?;
    Ok(())
}

pub async fn process_timers(pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
    rotate_jwt_secret(&mut db).await?;
    rotate_encryption_key(&mut db).await?;
    let chats: Vec<Chat> = chats::table
        .filter(chats::chat_type.eq(ChatType::NonInteractive as i32).and(chats::refresh_consecutive_llm_messages.le(SystemTime::now())))
        .get_results(&mut db)
        .await?;
    for chat in chats {
        diesel::update(chats::table.find(chat.id.ok_or("Invalid chat")?))
            .set((
                chats::consecutive_llm_messages.eq(0),
                chats::refresh_consecutive_llm_messages.eq(SystemTime::now().checked_add(Duration::from_hours(24)).ok_or("Error calculating time")?),
            ))
            .execute(&mut db)
            .await
            .ok();
    }
    Ok(())
}
