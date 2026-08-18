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
    let mut toolset = ToolSet::default();
    toolset.add_tool(DeviceInfoTool {});
    toolset.add_tool(EmailTool { user });
    toolset.add_tool(ExecuteCodeTool { user, chat, sequence, tool_call_id });
    toolset.add_tool(PreapproveCodeTool { chat });
    toolset.add_tool(ThinkTool {});
    toolset.add_tool(TimeTool {});
    toolset.add_tool(SleepTool {});
    toolset.add_tool(CompactTool { chat, sequence });
    toolset.add_tool(SummaryTool { chat });
    toolset
}
