use crate::{
    auth::authenticate,
    get_db,
    util::{convert_timestamp, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use diesel::{dsl::max, prelude::*};
use serde::{Deserialize, Serialize};
use shared_api::{ChatType, chat_type_to_str};
use shared_server::{
    models::{Chat, Message, ToolCall},
    notify_server,
    schema::{chats, messages, tool_calls},
};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

#[derive(Serialize)]
pub struct APIChat {
    id: Uuid,
    summary: String,
    start_timestamp: u64,
    end_timestamp: u64,
    chat_type: String,
}
#[derive(Serialize)]
pub struct APIToolCall {
    call_id: String,
    tool: String,
    request: String,
    response: Option<String>,
    user_consented: Option<bool>,
}
#[derive(Serialize)]
pub struct APIMessage {
    sequence: i32,
    content: String,
    sent_on: u64,
    user_message: bool,
    finished: bool,
    reasoning: Option<String>,
    tool_calls: Vec<APIToolCall>,
    input_tokens: u64,
    output_tokens: u64,
    model: String,
}
#[derive(Serialize)]
pub struct APIChatDetailed {
    id: Uuid,
    summary: String,
    start_timestamp: u64,
    end_timestamp: u64,
    messages: Vec<APIMessage>,
    chat_type: String,
}
#[derive(Serialize)]
pub struct CreateChatResponse {
    id: Uuid,
}
#[derive(Deserialize)]
pub struct SendMessageRequest {
    content: String,
}
#[derive(Deserialize)]
pub struct ManageToolCallConsentRequest {
    approved: bool,
    rejection_reason: String,
}
#[derive(Deserialize)]
pub struct CreateChatRequest {
    chat_type: String,
}

pub async fn chats_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<Json<Vec<APIChat>>, StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "GET")?;
    let mut db = get_db()?;
    let chats: Vec<Chat> = chats::table
        .filter(chats::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .get_results(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let chats: Result<Vec<APIChat>, StatusCode> = chats
        .iter()
        .map(|chat| {
            Ok(APIChat {
                id: chat.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
                summary: chat.summary.clone(),
                start_timestamp: convert_timestamp(chat.start_timestamp)?,
                end_timestamp: convert_timestamp(chat.end_timestamp)?,
                chat_type: chat_type_to_str(chat.chat_type).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
            })
        })
        .collect();
    Ok(Json(chats?))
}
pub async fn create_chat(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<Json<CreateChatResponse>, StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "POST")?;
    let request: CreateChatRequest = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let chat_type = match request.chat_type.as_str() {
        "interactive" => ChatType::Interactive,
        "noninteractive" => ChatType::NonInteractive,
        _ => {
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    let mut refresh_time = None;
    if chat_type == ChatType::NonInteractive {
        refresh_time = Some(SystemTime::now().checked_add(Duration::from_hours(24)).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    }
    let mut db = get_db()?;
    let mut chat = Chat {
        id: None,
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        summary: "Empty Chat".to_string(),
        start_timestamp: SystemTime::now(),
        end_timestamp: SystemTime::now(),
        error_count: 0,
        consecutive_llm_messages: 0,
        max_consecutive_llm_messages: 20,
        chat_type: chat_type as i32,
        refresh_consecutive_llm_messages: refresh_time,
    };
    chat = diesel::insert_into(chats::table).values(chat).get_result(&mut db).or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(Json(CreateChatResponse {
        id: chat.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    }))
}
pub async fn delete_chat(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "DELETE")?;
    let mut db = get_db()?;
    diesel::delete(
        chats::table.filter(
            chats::id
                .eq(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?)
                .and(chats::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        ),
    )
    .execute(&mut db)
    .or(Err(StatusCode::NOT_FOUND))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
pub async fn get_chat(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<Json<APIChatDetailed>, StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "GET")?;
    let mut db = get_db()?;
    let chat: Chat = chats::table
        .filter(
            chats::id
                .eq(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?)
                .and(chats::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        )
        .get_result(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    let messages: Vec<Message> = messages::table
        .filter(messages::chat.eq(chat.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .order_by(messages::sequence.asc())
        .get_results(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let messages: Result<Vec<APIMessage>, StatusCode> = messages
        .iter()
        .map(|message| {
            let tool_calls: Vec<ToolCall> = tool_calls::table
                .filter(tool_calls::chat.eq(message.chat).and(tool_calls::sequence.eq(message.sequence)))
                .get_results(&mut db)
                .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
            let tool_calls = tool_calls
                .iter()
                .map(|call| APIToolCall {
                    call_id: call.call_id.clone(),
                    tool: call.tool.clone(),
                    request: call.request.clone(),
                    response: call.response.clone(),
                    user_consented: call.user_consented,
                })
                .collect();
            Ok(APIMessage {
                sequence: message.sequence,
                content: message.content.clone(),
                sent_on: convert_timestamp(message.sent_on)?,
                user_message: message.user_message,
                finished: message.finished,
                reasoning: message.reasoning.clone(),
                tool_calls,
                input_tokens: message.input_tokens as u64,
                output_tokens: message.output_tokens as u64,
                model: message.model.clone(),
            })
        })
        .collect();
    let chat = APIChatDetailed {
        id: chat.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        summary: chat.summary,
        start_timestamp: convert_timestamp(chat.start_timestamp)?,
        end_timestamp: convert_timestamp(chat.end_timestamp)?,
        messages: messages?,
        chat_type: chat_type_to_str(chat.chat_type).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
    };
    Ok(Json(chat))
}
pub async fn send_message(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "POST")?;
    let request: SendMessageRequest = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let chat: Chat = chats::table
        .filter(
            chats::id
                .eq(Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?)
                .and(chats::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)),
        )
        .get_result(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    let chat_id = chat.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let last_sequence: Option<i32> = messages::table
        .select(max(messages::sequence))
        .filter(messages::chat.eq(chat_id))
        .first(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let last_sequence = last_sequence.unwrap_or(-1);
    if last_sequence != -1 {
        let last_message: Message = messages::table
            .filter(messages::chat.eq(chat_id).and(messages::sequence.eq(last_sequence)))
            .first(&mut db)
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        if last_message.user_message == last_message.finished {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    let content = request.content.trim().to_string();
    if content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let message = Message {
        chat: chat_id,
        sequence: last_sequence + 1,
        content,
        sent_on: SystemTime::now(),
        user_message: true,
        generation_id: None,
        finished: true,
        reasoning: None,
        input_tokens: 0,
        output_tokens: 0,
        model: "".to_owned(),
    };
    diesel::update(chats::table.find(chat_id))
        .set((chats::end_timestamp.eq(SystemTime::now()), chats::error_count.eq(0), chats::consecutive_llm_messages.eq(0)))
        .execute(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    diesel::insert_into(messages::table).values(message).execute(&mut db).or(Err(StatusCode::BAD_REQUEST))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("MASTER:LLM").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
pub async fn manage_tool_call_consent(headers: HeaderMap, Path((id, seq, call_id)): Path<(String, i32, String)>, uri: OriginalUri, body: Bytes) -> Result<(), StatusCode> {
    let user = authenticate(&headers)?.0;
    validate_signature(&headers, uri.0, &body, "POST")?;
    let request: ManageToolCallConsentRequest = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let chat_uuid = Uuid::parse_str(&id).or(Err(StatusCode::BAD_REQUEST))?;
    let _chat: Chat = chats::table
        .filter(chats::id.eq(chat_uuid).and(chats::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?)))
        .get_result(&mut db)
        .or(Err(StatusCode::NOT_FOUND))?;
    if request.approved {
        diesel::update(tool_calls::table.filter(tool_calls::chat.eq(chat_uuid).and(tool_calls::sequence.eq(seq)).and(tool_calls::call_id.eq(call_id))))
            .set(tool_calls::user_consented.eq(true))
            .execute(&mut db)
            .or(Err(StatusCode::NOT_FOUND))?;
        notify_server("MASTER:TOOLS").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    } else {
        diesel::update(tool_calls::table.filter(tool_calls::chat.eq(chat_uuid).and(tool_calls::sequence.eq(seq)).and(tool_calls::call_id.eq(call_id))))
            .set((
                tool_calls::user_consented.eq(false),
                tool_calls::response.eq(format!("User has rejected the tool call: {}", request.rejection_reason)),
            ))
            .execute(&mut db)
            .or(Err(StatusCode::NOT_FOUND))?;
        notify_server("MASTER:LLM").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    }
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(())
}
