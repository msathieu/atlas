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
    pub device_info_type: Option<String>,
    pub chat_id: Option<Uuid>,
    pub chat_sequence: Option<i32>,
    pub chat_tool_call_id: Option<String>,
    pub initiated: bool,
    pub language: i32,
    pub transient: bool,
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
    pub container_id: Option<Uuid>,
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
    pub enabled: bool,
    pub password_enabled: bool,
    pub last_login: Option<SystemTime>,
    pub email: Option<String>,
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
    pub chat_type: i32,
    pub refresh_consecutive_llm_messages: Option<SystemTime>,
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
    pub provider: String,
    pub model: String,
    pub compacted: bool,
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
    pub initiated: bool,
    pub provider_id: Option<String>,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::preapproved_code)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PreapprovedCode {
    #[diesel(deserialize_as = Uuid)]
    pub id: Option<Uuid>,
    pub chat: Uuid,
    pub code: String,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::passkeys)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Passkey {
    pub id: Vec<u8>,
    pub uid: Uuid,
    pub passkey: String,
    pub created_at: SystemTime,
    pub last_used: Option<SystemTime>,
    pub name: String,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::containers)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Container {
    #[diesel(deserialize_as = Uuid)]
    pub id: Option<Uuid>,
    pub uid: Uuid,
    pub origin: i32,
    pub state: i32,
    pub created_at: SystemTime,
    pub image: String,
    pub requested_state: i32,
    pub storage_size: i32,
    pub memory_limit: i32,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::development_environments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DevelopmentEnvironment {
    pub id: Uuid,
    pub name: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub nested_containers: bool,
    pub ssh_key: String,
    pub command: Option<Uuid>,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::user_settings)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserSetting {
    pub uid: Uuid,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Queryable, Insertable, Selectable, AsChangeset)]
#[diesel(table_name = crate::schema::agents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub command: Option<Uuid>,
    pub password: String,
}
