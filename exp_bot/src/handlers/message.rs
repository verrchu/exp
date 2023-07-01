use anyhow::Context;
use chrono::NaiveDate;
use regex::Regex;
use rust_decimal::Decimal;
use teloxide_core::{
    payloads::SendMessageSetters,
    requests::Requester,
    types::{InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup},
};

use crate::{ConversationState, ExecCtx, MsgCtx};

pub(crate) async fn report(exec_ctx: &ExecCtx, msg_ctx: &MsgCtx) -> anyhow::Result<()> {
    let make_row = |months: [&str; 4]| {
        months.map(|m| {
            InlineKeyboardButton::new(m, InlineKeyboardButtonKind::CallbackData(m.to_string()))
        })
    };

    exec_ctx
        .bot
        .send_message(msg_ctx.chat.id, "2023".to_string())
        .reply_markup(InlineKeyboardMarkup::new([
            make_row(["Jan", "Feb", "Mar", "Apr"]),
            make_row(["May", "Jun", "Jul", "Aug"]),
            make_row(["Sep", "Oct", "Nov", "Dec"]),
        ]))
        .await
        .context("failed to send message")?;

    Ok(())
}

pub(crate) async fn add_expense(exec_ctx: &ExecCtx, msg_ctx: &MsgCtx) -> anyhow::Result<()> {
    exec_ctx
        .bot
        .send_message(msg_ctx.chat.id, "choose category")
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "add category",
            InlineKeyboardButtonKind::CallbackData("add_category".into()),
        )]]))
        .await
        .context("failed to send message")?;

    Ok(())
}

pub(crate) async fn category_name(exec_ctx: &ExecCtx, msg_ctx: &MsgCtx) -> anyhow::Result<()> {
    // unwrap: if we got this far then the message definetely contains text
    let cname = msg_ctx.msg.text().unwrap();

    exec_ctx
        .bot
        .send_message(msg_ctx.chat.id, format!("[category confirmation]: {cname}"))
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::new(
                "confirm",
                InlineKeyboardButtonKind::CallbackData(format!("ccn:{}", msg_ctx.msg.id.0)),
            ),
            InlineKeyboardButton::new(
                "reject",
                InlineKeyboardButtonKind::CallbackData(format!("rcn:{}", msg_ctx.msg.id.0)),
            ),
        ]]))
        .await
        .context("failed to send message")?;

    exec_ctx
        .cstate
        .set(
            msg_ctx.user.id,
            ConversationState::AwaitingCategoryNameConfirmation {
                msg_id: msg_ctx.msg.id,
                category_name: cname.to_string(),
            },
        )
        .await;

    Ok(())
}

pub(crate) async fn expense_amount(
    exec_ctx: &ExecCtx,
    msg_ctx: &MsgCtx,
    cname: &str,
    date: NaiveDate,
) -> anyhow::Result<()> {
    // unwrap: if we got this far then the message definetely contains text
    let amount = msg_ctx.msg.text().unwrap();
    let pattern = Regex::new(r"^(0|[^0](\d+)?)([.,]\d{1,2})?$").unwrap();

    if !pattern.is_match(amount) {
        exec_ctx
            .bot
            .send_message(
                msg_ctx.chat.id,
                "invalid expense amount. try again".to_string(),
            )
            .await
            .context("failed to send message")?;

        return Ok(());
    }

    let amount = Decimal::from_str_exact(amount).context("failed to parse expense amount")?;

    exec_ctx
        .storage
        .add_expense(&msg_ctx.user, cname, amount, date)
        .await
        .context("failed to add expense")?;

    exec_ctx
        .bot
        .send_message(msg_ctx.chat.id, "expense added")
        .await
        .context("failed to send message")?;

    exec_ctx.cstate.clear(msg_ctx.user.id).await;

    Ok(())
}
