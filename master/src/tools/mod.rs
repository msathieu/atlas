use crate::tools::{
    compact::CompactTool, device_info::DeviceInfoTool, email::EmailTool, execute_code::ExecuteCodeTool, preapprove_code::PreapproveCodeTool, sleep::SleepTool, summary::SummaryTool, time::TimeTool,
};
use rig_agent::tool::ToolSet;
use rig_core::tool::builtin::ThinkTool;
use uuid::Uuid;

pub mod compact;
pub mod device_info;
pub mod email;
pub mod execute_code;
pub mod preapprove_code;
pub mod sleep;
pub mod summary;
pub mod time;

pub fn get_toolset(user: Uuid, chat: Uuid, sequence: i32, tool_call_id: String) -> ToolSet {
    ToolSet::builder()
        .static_tool(DeviceInfoTool {})
        .static_tool(EmailTool {})
        .static_tool(ExecuteCodeTool { user, chat, sequence, tool_call_id })
        .static_tool(PreapproveCodeTool { chat })
        .static_tool(ThinkTool {})
        .static_tool(TimeTool {})
        .static_tool(SleepTool {})
        .static_tool(CompactTool { chat, sequence })
        .static_tool(SummaryTool { chat })
        .build()
}
