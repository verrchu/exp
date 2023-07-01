use std::str::FromStr;

use anyhow::Context;
use teloxide_core::types::MessageId;

pub enum Command {
    AddCategory,
    ConfirmCategoryName { msg_id: MessageId },
}

impl FromStr for Command {
    type Err = anyhow::Error;

    fn from_str(cmd: &str) -> anyhow::Result<Command> {
        if cmd == "add_category" {
            return Ok(Command::AddCategory);
        }

        if let Some(msg_id) = cmd.strip_prefix("ccn:") {
            let msg_id = msg_id.parse::<i32>().context(format!(
                "failed to parse message id (code: ccn, msg_id: {msg_id})"
            ))?;

            return Ok(Command::ConfirmCategoryName {
                msg_id: MessageId(msg_id),
            });
        }

        anyhow::bail!("unknown cmd: {cmd}");
    }
}
