use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl, pooled_connection::deadpool::Pool};
use shared_api::ChatType;
use shared_server::models::Chat;
use shared_server::schema::chats;
use std::error::Error;
use std::time::{Duration, SystemTime};

pub async fn process_timers(pool: Pool<AsyncPgConnection>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get().await?;
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
