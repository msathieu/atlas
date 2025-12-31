use std::time::SystemTime;

use diesel::prelude::*;
use uuid::Uuid;

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::commands)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Command {
    #[diesel(deserialize_as = Uuid)]
    pub id: Option<Uuid>,
    pub device: Uuid,
    pub uid: Option<Uuid>,
    pub code: String,
    pub completed: bool,
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub requested_timestamp: SystemTime,
    pub completed_timestamp: Option<SystemTime>,
    pub origin: i32,
    pub origin_id: String,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::devices)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Device {
    #[diesel(deserialize_as = Uuid)]
    pub id: Option<Uuid>,
    pub arch: String,
    pub os: String,
    pub version: String,
    pub pubkey: Vec<u8>,
    pub approved: bool,
    pub hostname: String,
    pub first_connection: SystemTime,
    pub last_connection: SystemTime,
    pub ip: String,
    pub agent_hash: Vec<u8>,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::device_info)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DeviceInfo {
    pub device: Uuid,
    pub info_type: String,
    pub success: bool,
    pub data: String,
    pub updated_timestamp: SystemTime,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::settings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Setting {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    #[diesel(deserialize_as = Uuid)]
    pub id: Option<Uuid>,
    pub username: String,
    pub password: String,
    pub jwt_counter: i32,
    pub admin: bool,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Chat {
    #[diesel(deserialize_as = Uuid)]
    pub id: Option<Uuid>,
    pub uid: Uuid,
    pub summary: String,
    pub start_timestamp: SystemTime,
    pub end_timestamp: SystemTime,
    pub error_count: i32,
    pub consecutive_llm_messages: i32,
    pub max_consecutive_llm_messages: i32,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::messages)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Message {
    pub chat: Uuid,
    pub sequence: i32,
    pub content: String,
    pub sent_on: SystemTime,
    pub user_message: bool,
    pub generation_id: Option<String>,
    pub finished: bool,
    pub reasoning: Option<String>,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub model: String,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::tool_calls)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ToolCall {
    pub chat: Uuid,
    pub sequence: i32,
    pub call_id: String,
    pub tool: String,
    pub request: String,
    pub response: Option<String>,
    pub user_consented: Option<bool>,
    pub error_count: i32,
}
