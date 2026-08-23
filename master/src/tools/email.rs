use crate::DB_POOL;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use rig_core::tool::{PortableTool, ToolExecutionError};
use serde::Deserialize;
use serde_json::json;
use shared_server::{get_user_setting, models::Setting, schema::settings};
use uuid::Uuid;

pub struct EmailTool {
    pub user: Uuid,
}

#[derive(Deserialize)]
pub struct EmailArgs {
    subject: String,
    body: String,
}

impl PortableTool for EmailTool {
    const NAME: &str = "email";
    type Error = ToolExecutionError;
    type Args = EmailArgs;
    type Output = String;

    fn description(&self) -> String {
        "Send an email to the user".into()
    }
    fn parameters(&self) -> serde_json::Value {
        json!({
        "type": "object",
            "properties": {
              "subject": {"type": "string", "description": "The subject of the mail"},
              "body": {"type": "string", "description": "The body of the mail"}
            },
            "required": ["subject", "body"]
          })
    }
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let pool = DB_POOL.get().ok_or(ToolExecutionError::other("Database connection failed"))?;
        let mut db = pool.get().await.map_err(|_| ToolExecutionError::other("Database connection failed"))?;
        let mail_server: Setting = settings::table
            .find("mail_server")
            .get_result(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Couldn't get email settings"))?;
        let mail_username: Setting = settings::table
            .find("mail_username")
            .get_result(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Couldn't get email settings"))?;
        let mail_password: Setting = settings::table
            .find("mail_password")
            .get_result(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Couldn't get email settings"))?;
        let mail_from: Setting = settings::table
            .find("mail_from")
            .get_result(&mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Couldn't get email settings"))?;
        let mail_to = get_user_setting(self.user, "mail_to".to_string(), &mut db)
            .await
            .map_err(|_| ToolExecutionError::other("Couldn't get email settings"))?;
        if mail_server.value.is_empty() || mail_username.value.is_empty() || mail_password.value.is_empty() || mail_from.value.is_empty() || mail_to.value.is_empty() {
            return Err(ToolExecutionError::other("Mail server isn't configured"));
        }
        let message = Message::builder()
            .from(Mailbox {
                name: Some("Atlas".to_string()),
                email: mail_from.value.parse().map_err(|_| ToolExecutionError::other("Invalid from address"))?,
            })
            .to(Mailbox {
                name: None,
                email: mail_to.value.parse().map_err(|_| ToolExecutionError::other("Invalid to address"))?,
            })
            .header(ContentType::TEXT_PLAIN)
            .message_id(None)
            .subject(args.subject)
            .body(args.body)
            .map_err(|_| ToolExecutionError::other("Invalid email"))?;
        let transport: AsyncSmtpTransport<Tokio1Executor> = AsyncSmtpTransport::<Tokio1Executor>::relay(&mail_server.value)
            .map_err(|_| ToolExecutionError::other("Invalid mail server"))?
            .credentials(Credentials::new(mail_username.value, mail_password.value))
            .build();
        transport.send(message).await.map_err(|_| ToolExecutionError::other("Couldn't send mail"))?;
        Ok("OK".into())
    }
}
