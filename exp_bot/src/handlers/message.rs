use anyhow::Context;
use regex::Regex;
use rust_decimal::Decimal;
use teloxide_core::{
    payloads::SendMessageSetters,
    requests::Requester,
    types::{
        ChatId, InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, Message,
        UserId,
    },
};

use crate::{models, storage, ConversationState, ExecCtx};

pub(crate) async fn report(ctx: &ExecCtx, chat_id: ChatId, year: i32) -> anyhow::Result<()> {
    let make_row = |months: [&str; 4]| {
        months.map(|m| {
            InlineKeyboardButton::new(m, InlineKeyboardButtonKind::CallbackData(m.to_string()))
        })
    };

    ctx.bot
        .send_message(chat_id, format!("{year}"))
        .reply_markup(InlineKeyboardMarkup::new([
            make_row(["Jan", "Feb", "Mar", "Apr"]),
            make_row(["May", "Jun", "Jul", "Aug"]),
            make_row(["Sep", "Oct", "Nov", "Dec"]),
        ]))
        .await
        .context("failed to send message")?;

    Ok(())
}

pub(crate) async fn add_expense(ctx: &ExecCtx, chat_id: ChatId) -> anyhow::Result<()> {
    ctx.bot
        .send_message(chat_id, "choose category")
        .reply_markup(InlineKeyboardMarkup::new([[InlineKeyboardButton::new(
            "add category",
            InlineKeyboardButtonKind::CallbackData("add_category".into()),
        )]]))
        .await
        .context("failed to send message")?;

    Ok(())
}

pub(crate) async fn category_name(
    ctx: &ExecCtx,
    (user_id, chat_id): (UserId, ChatId),
    msg: &Message,
) -> anyhow::Result<()> {
    // unwrap: if we got this far then the message definetely contains text
    let cname = msg.text().unwrap();

    ctx.bot
        .send_message(chat_id, format!("[category confirmation]: {cname}"))
        .reply_markup(InlineKeyboardMarkup::new([[
            InlineKeyboardButton::new(
                "confirm",
                InlineKeyboardButtonKind::CallbackData(format!("ccn:{}", msg.id.0)),
            ),
            InlineKeyboardButton::new(
                "abort",
                InlineKeyboardButtonKind::CallbackData(format!("acn:{}", msg.id.0)),
            ),
        ]]))
        .await
        .context("failed to send message")?;

    ctx.cstate
        .set(
            user_id,
            ConversationState::AwaitingCategoryNameConfirmation {
                msg_id: msg.id,
                category_name: cname.to_string(),
            },
        )
        .await;

    Ok(())
}

pub(crate) async fn expense_amount(
    ctx: &ExecCtx,
    (user_id, chat_id): (UserId, ChatId),
    msg: &Message,
    cname: &str,
) -> anyhow::Result<()> {
    // unwrap: if we got this far then the message definetely contains text
    let amount = msg.text().unwrap();
    let pattern = Regex::new(r"^(0|[^0](\d+)?)([.,]\d{1,2})?$").unwrap();

    if !pattern.is_match(amount) {
        ctx.bot
            .send_message(chat_id, format!("invalid expense amount. try again"))
            .await
            .context("failed to send message")?;
    }

    let amount = Decimal::from_str_exact(amount).context("failed to parse expense amount")?;

    storage::user::add_expense(
        models::User { id: user_id.0 },
        cname,
        amount,
        &ctx.db_client,
    )
    .await
    .context("failed to add expense")?;

    ctx.bot
        .send_message(chat_id, "expense added")
        .await
        .context("failed to send message")?;

    ctx.cstate.clear(user_id).await;

    Ok(())
}
