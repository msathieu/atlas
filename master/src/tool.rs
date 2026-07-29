use crate::tools::get_toolset;
use diesel::{
    PgConnection,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use rig_agent::tool::ToolContext;
use shared_server::{
    models::{Chat, ToolCall},
    notify_server,
    schema::{chats, tool_calls},
};
use std::error::Error;

pub async fn process_tool_calls(pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    let tool_calls: Vec<ToolCall> = tool_calls::table
        .filter(tool_calls::response.is_null().and(tool_calls::user_consented.eq(true)).and(tool_calls::initiated.eq(false)))
        .get_results(&mut db)?;
    for call in tool_calls {
        let pool = pool.clone();
        diesel::update(
            tool_calls::table.filter(
                tool_calls::chat
                    .eq(call.chat)
                    .and(tool_calls::sequence.eq(call.sequence))
                    .and(tool_calls::call_id.eq(call.call_id.clone())),
            ),
        )
        .set(tool_calls::initiated.eq(true))
        .execute(&mut db)?;
        tokio::spawn(async {
            if let Err(e) = process_tool_call(call, pool).await {
                eprintln!("Error while processing tool calls: {}", e);
            }
        });
    }
    Ok(())
}
async fn process_tool_call(call: ToolCall, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    let chat: Chat = chats::table.filter(chats::id.eq(call.chat)).get_result(&mut db)?;
    let toolset = get_toolset(chat.uid, call.chat, call.sequence, call.call_id.clone());
    let execution = toolset.execute(&call.tool, call.request, &mut ToolContext::new()).await;
    let text = if let Some(json) = execution.output().as_json() {
        json.to_string()
    } else if let Some(output) = execution.output().as_text() {
        output.to_string()
    } else {
        "Error while executing tool".to_string()
    };
    diesel::update(tool_calls::table.filter(tool_calls::chat.eq(call.chat).and(tool_calls::sequence.eq(call.sequence)).and(tool_calls::call_id.eq(call.call_id))))
        .set(tool_calls::response.eq(Some(text)))
        .execute(&mut db)?;
    notify_server("MASTER:LLM").await?;
    Ok(())
}
