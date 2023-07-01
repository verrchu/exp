use std::str::FromStr;

use anyhow::Context;
use chrono::{Days, NaiveDate, Utc};
use teloxide_core::types::MessageId;

pub enum Command {
    AddCategory,
    ConfirmCategoryName { msg_id: MessageId },
    RejectCategoryName { msg_id: MessageId },
    PickExpenseDate { msg_id: MessageId, date: NaiveDate },
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

        if let Some(data) = cmd.strip_prefix("rcn:") {
            let msg_id = data.parse::<i32>().context(format!(
                "failed to parse message id (code: rcn, data: {data})"
            ))?;

            return Ok(Command::RejectCategoryName {
                msg_id: MessageId(msg_id),
            });
        }

        if let Some(data) = cmd.strip_prefix("ped:") {
            let mut chunks = data.split(':');

            let msg_id = chunks.next().unwrap();
            let msg_id = msg_id.parse::<i32>().context(format!(
                "failed to parse message id (code: ped, data: {data})",
            ))?;

            let kind = chunks.next().unwrap();
            if kind == "today" {
                let date = Utc::now().date_naive();
                return Ok(Command::PickExpenseDate {
                    msg_id: MessageId(msg_id),
                    date,
                });
            }

            if kind == "yesterday" {
                let date = Utc::now().date_naive() - Days::new(1);
                return Ok(Command::PickExpenseDate {
                    msg_id: MessageId(msg_id),
                    date,
                });
            }

            anyhow::bail!("failed to parse command (code: ped, data: {data})");
        }

        anyhow::bail!("unknown cmd: {cmd}");
    }
}
