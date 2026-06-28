use crate::tools::{device_info::DeviceInfoTool, email::EmailTool, execute_code::ExecuteCodeTool, preapprove_code::PreapproveCodeTool, sleep::SleepTool, time::TimeTool};
use diesel::{
    PgConnection,
    prelude::*,
    r2d2::{ConnectionManager, Pool},
};
use rig_core::{tool::ToolDyn, tools::ThinkTool};
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
    let mut tasks = Vec::new();
    for call in tool_calls {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async {
            if let Err(e) = process_tool_call(call, pool).await {
                println!("Error while processing tool calls: {}", e);
            }
        }));
    }
    for i in tasks {
        if let Err(e) = i.await {
            println!("Error while processing tool calls: {}", e);
        }
    }
    Ok(())
}
async fn process_tool_call(call: ToolCall, pool: Pool<ConnectionManager<PgConnection>>) -> Result<(), Box<dyn Error>> {
    let mut db = pool.get()?;
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
    let chat: Chat = chats::table.filter(chats::id.eq(call.chat)).get_result(&mut db)?;
    let mut incorrect_tool = false;
    let tool: Box<dyn ToolDyn> = match call.tool.as_str() {
        "device_info" => Box::new(DeviceInfoTool {}),
        "email" => Box::new(EmailTool {}),
        "execute_code" => Box::new(ExecuteCodeTool {
            user: chat.uid,
            chat: call.chat,
            sequence: call.sequence,
            tool_call_id: call.call_id.clone(),
        }),
        "preapprove_code" => Box::new(PreapproveCodeTool { chat: call.chat }),
        "think" => Box::new(ThinkTool {}),
        "time" => Box::new(TimeTool {}),
        "sleep" => Box::new(SleepTool {}),
        _ => {
            incorrect_tool = true;
            Box::new(ThinkTool {})
        }
    };
    let text = if incorrect_tool {
        "Tool doesn't exist".to_string()
    } else {
        let result = tool.call(call.request).await;
        match result {
            Ok(result) => result,
            Err(e) => format!("Error while executing tool: {}", e),
        }
    };
    diesel::update(tool_calls::table.filter(tool_calls::chat.eq(call.chat).and(tool_calls::sequence.eq(call.sequence)).and(tool_calls::call_id.eq(call.call_id))))
        .set(tool_calls::response.eq(Some(text)))
        .execute(&mut db)?;
    notify_server("MASTER:LLM").await?;
    Ok(())
}
