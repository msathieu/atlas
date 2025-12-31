use crate::tools::{device_info::DeviceInfoTool, execute_code::ExecuteCodeTool, sleep::SleepTool};
use diesel::{
    PgConnection,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use rig::{tool::ToolDyn, tools::ThinkTool};
use shared_server::{
    models::{Chat, ToolCall},
    notify_server,
    schema::{chats, tool_calls},
};
use std::error::Error;

pub async fn process_tool_calls(pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    let tool_calls: Vec<ToolCall> = tool_calls::table.filter(tool_calls::response.is_null().and(tool_calls::user_consented.eq(true))).get_results(&mut db)?;
    let mut tasks = Vec::new();
    for call in tool_calls {
        let pool = pool.clone();
        let error_count = call.error_count;
        let chat = call.chat;
        let sequence = call.sequence;
        let call_id = call.call_id.clone();
        tasks.push(tokio::spawn(async move {
            if let Err(e) = process_tool_call(call, pool.clone()).await {
                println!("Error while running tasks: {}", e);
                if let Ok(mut db) = pool.get() {
                    diesel::update(tool_calls::table.filter(tool_calls::chat.eq(chat).and(tool_calls::sequence.eq(sequence)).and(tool_calls::call_id.eq(call_id))))
                        .set(tool_calls::error_count.eq(error_count + 1))
                        .execute(&mut db)
                        .ok();
                }
            }
        }));
    }
    for i in tasks {
        if let Err(e) = i.await {
            println!("Error while running tasks: {}", e);
        }
    }
    Ok(())
}
async fn process_tool_call(call: ToolCall, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
    if call.error_count >= 5 {
        diesel::update(tool_calls::table.filter(tool_calls::chat.eq(call.chat).and(tool_calls::sequence.eq(call.sequence)).and(tool_calls::call_id.eq(call.call_id))))
            .set(tool_calls::response.eq(Some("Failed to execute tool: retry count exceeded")))
            .execute(&mut db)?;
        notify_server("MASTER:LLM").await?;
        return Ok(());
    }
    let chat: Chat = chats::table.filter(chats::id.eq(call.chat)).get_result(&mut db)?;
    let tool: Box<dyn ToolDyn> = match call.tool.as_str() {
        "device_info" => Box::new(DeviceInfoTool {}),
        "execute_code" => Box::new(ExecuteCodeTool { user: chat.uid }),
        "think" => Box::new(ThinkTool {}),
        "sleep" => Box::new(SleepTool {}),
        _ => return Err("Invalid tool".into()),
    };
    let result = tool.call(call.request).await;
    let text = match result {
        Ok(result) => result,
        Err(e) => format!("Error while executing tool: {}", e),
    };
    diesel::update(tool_calls::table.filter(tool_calls::chat.eq(call.chat).and(tool_calls::sequence.eq(call.sequence)).and(tool_calls::call_id.eq(call.call_id))))
        .set(tool_calls::response.eq(Some(text)))
        .execute(&mut db)?;
    notify_server("MASTER:LLM").await?;
    Ok(())
}
