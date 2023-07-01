use anyhow::Context;
use chrono::NaiveDate;
use teloxide_core::{
    payloads::SendMessageSetters,
    requests::Requester,
    types::{
        ChatId, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, Message,
        MessageId, UserId,
    },
};

use crate::{storage, ConversationState, ExecCtx, MsgCtx};

pub(crate) async fn add_category(exec_ctx: &ExecCtx, msg_ctx: &MsgCtx) -> anyhow::Result<()> {
    exec_ctx
        .bot
        .send_message(msg_ctx.chat.id, "please, provide category name")
        .await
        .context("failed to send message")?;
    exec_ctx
        .cstate
        .set(msg_ctx.user.id, ConversationState::AwaitingCategoryName)
        .await;

    Ok(())
}

pub(crate) async fn reject_category_name(
    source_msg_id: MessageId,
    exec_ctx: &ExecCtx,
    msg_ctx: &MsgCtx,
) -> anyhow::Result<()> {
    if let Some(ConversationState::AwaitingCategoryNameConfirmation {
        msg_id: expected_msg_id,
        ..
    }) = exec_ctx.cstate.get(msg_ctx.user.id).await
    {
        if source_msg_id == expected_msg_id {
            exec_ctx
                .bot
                .send_message(msg_ctx.chat.id, "choose category")
                .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
                    "add category",
                    InlineKeyboardButtonKind::CallbackData("add_category".into()),
                )]]))
                .await
                .context("failed to send message")?;

            exec_ctx.cstate.clear(msg_ctx.user.id).await;
        }
    }

    Ok(())
}

pub(crate) async fn confirm_category_name(
    source_msg_id: MessageId,
    exec_ctx: &ExecCtx,
    msg_ctx: &MsgCtx,
) -> anyhow::Result<()> {
    if let Some(ConversationState::AwaitingCategoryNameConfirmation {
        msg_id: expected_msg_id,
        category_name: cname,
    }) = exec_ctx.cstate.get(msg_ctx.user.id).await
    {
        if source_msg_id == expected_msg_id {
            let inserted = storage::user::add_category(&msg_ctx.user, &cname, &exec_ctx.db_client)
                .await
                .context("failed to add category")?;

            let mut resp = inserted
                .then(|| format!("category '{cname}' added"))
                .unwrap_or_else(|| format!("category '{cname}' has already been added"));

            resp.push_str("\n\nplease, provide expense date");

            let mk_button = |name: &str, data: &str| {
                InlineKeyboardButton::new(
                    name,
                    InlineKeyboardButtonKind::CallbackData(format!(
                        "ped:{}:{data}",
                        msg_ctx.msg.id
                    )),
                )
            };

            exec_ctx
                .bot
                .send_message(msg_ctx.chat.id, resp)
                .reply_markup(InlineKeyboardMarkup::new([
                    [mk_button("today", "today")],
                    [mk_button("yesterday", "yesterday")],
                    // [mk_button("pick date", "custom")],
                ]))
                .await
                .context("failed to send message")?;

            exec_ctx
                .cstate
                .set(
                    msg_ctx.user.id,
                    ConversationState::AwaitingExpenseDate {
                        msg_id: msg_ctx.msg.id,
                        category_name: cname.to_string(),
                    },
                )
                .await;
        }
    }

    Ok(())
}

pub(crate) async fn pick_expense_date(
    source_msg_id: MessageId,
    date: NaiveDate,
    exec_ctx: &ExecCtx,
    msg_ctx: &MsgCtx,
) -> anyhow::Result<()> {
    if let Some(ConversationState::AwaitingExpenseDate {
        msg_id: expected_msg_id,
        category_name: cname,
    }) = exec_ctx.cstate.get(msg_ctx.user.id).await
    {
        if source_msg_id == expected_msg_id {
            exec_ctx
                .bot
                .send_message(msg_ctx.chat.id, "please, provide expense amount")
                .await
                .context("failed to send message")?;

            exec_ctx
                .cstate
                .set(
                    msg_ctx.user.id,
                    ConversationState::AwaitingExpenseAmount {
                        date,
                        category_name: cname.to_string(),
                    },
                )
                .await;
        }
    }

    Ok(())
}
