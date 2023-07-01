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

use crate::{models, storage, ConversationState, ExecCtx};

pub(crate) async fn add_category(
    ctx: &ExecCtx,
    (user_id, chat_id): (UserId, ChatId),
) -> anyhow::Result<()> {
    ctx.bot
        .send_message(chat_id, "please, provide category name")
        .await
        .context("failed to send message")?;
    ctx.cstate
        .set(user_id, ConversationState::AwaitingCategoryName)
        .await;

    Ok(())
}

pub(crate) async fn reject_category_name(
    ctx: &ExecCtx,
    (user_id, chat_id): (UserId, ChatId),
    source_msg_id: MessageId,
) -> anyhow::Result<()> {
    if let Some(ConversationState::AwaitingCategoryNameConfirmation {
        msg_id: expected_msg_id,
        ..
    }) = ctx.cstate.get(user_id).await
    {
        if source_msg_id == expected_msg_id {
            ctx.bot
                .send_message(chat_id, "choose category")
                .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
                    "add category",
                    InlineKeyboardButtonKind::CallbackData("add_category".into()),
                )]]))
                .await
                .context("failed to send message")?;

            ctx.cstate.clear(user_id).await;
        }
    }

    Ok(())
}

pub(crate) async fn confirm_category_name(
    ctx: &ExecCtx,
    (user_id, chat_id): (UserId, ChatId),
    source_msg_id: MessageId,
    msg: &Message,
) -> anyhow::Result<()> {
    if let Some(ConversationState::AwaitingCategoryNameConfirmation {
        msg_id: expected_msg_id,
        category_name: cname,
    }) = ctx.cstate.get(user_id).await
    {
        if source_msg_id == expected_msg_id {
            let inserted =
                storage::user::add_category(models::User { id: user_id.0 }, &cname, &ctx.db_client)
                    .await
                    .context("failed to add category")?;

            let mut resp = inserted
                .then(|| format!("category '{cname}' added"))
                .unwrap_or_else(|| format!("category '{cname}' has already been added"));

            resp.push_str("\n\nplease, provide expense date");

            let mk_button = |name: &str, data: &str| {
                InlineKeyboardButton::new(
                    name,
                    InlineKeyboardButtonKind::CallbackData(format!("ped:{}:{data}", msg.id)),
                )
            };

            ctx.bot
                .send_message(chat_id, resp)
                .reply_markup(InlineKeyboardMarkup::new([
                    [mk_button("today", "today")],
                    [mk_button("yesterday", "yesterday")],
                    // [mk_button("pick date", "custom")],
                ]))
                .await
                .context("failed to send message")?;

            ctx.cstate
                .set(
                    user_id,
                    ConversationState::AwaitingExpenseDate {
                        msg_id: msg.id,
                        category_name: cname.to_string(),
                    },
                )
                .await;
        }
    }

    Ok(())
}

pub(crate) async fn pick_expense_date(
    ctx: &ExecCtx,
    (user_id, chat_id): (UserId, ChatId),
    date: NaiveDate,
    source_msg_id: MessageId,
) -> anyhow::Result<()> {
    if let Some(ConversationState::AwaitingExpenseDate {
        msg_id: expected_msg_id,
        category_name: cname,
    }) = ctx.cstate.get(user_id).await
    {
        if source_msg_id == expected_msg_id {
            ctx.bot
                .send_message(chat_id, "please, provide expense amount")
                .await
                .context("failed to send message")?;

            ctx.cstate
                .set(
                    user_id,
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
