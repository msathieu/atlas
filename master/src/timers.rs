use diesel::prelude::*;
use diesel::{
    PgConnection,
    r2d2::{ConnectionManager, Pool},
};
use shared_api::ChatType;
use shared_server::models::Chat;
use shared_server::schema::chats;
use std::error::Error;
use std::time::{Duration, SystemTime};

pub async fn process_timers(pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    let chats: Vec<Chat> = chats::table
        .filter(chats::chat_type.eq(ChatType::NonInteractive as i32).and(chats::refresh_consecutive_llm_messages.le(SystemTime::now())))
        .get_results(&mut db)?;
    for chat in chats {
        diesel::update(chats::table.find(chat.id.ok_or("Invalid chat")?))
            .set((
                chats::consecutive_llm_messages.eq(0),
                chats::refresh_consecutive_llm_messages.eq(SystemTime::now().checked_add(Duration::from_hours(24)).ok_or("Error calculating time")?),
            ))
            .execute(&mut db)
            .ok();
    }
    Ok(())
}
