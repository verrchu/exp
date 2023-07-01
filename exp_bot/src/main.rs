mod conversation_state;
use conversation_state::{ConversationState, ConversationStates};
mod command;
use command::Command;
mod handlers;
mod models;
mod storage;

use std::{env::var, time::Duration};

use anyhow::Context;
use chrono::{Datelike, Utc};
use teloxide_core::{
    payloads::GetUpdatesSetters,
    requests::Requester,
    types::{AllowedUpdate, CallbackQuery, Chat, Message, UpdateKind, User},
    Bot,
};
use tokio::time::{interval, MissedTickBehavior};
use tokio_postgres::{Client as PgClient, Config as PgConf, NoTls};

struct ExecCtx {
    bot: Bot,
    db_client: PgClient,
    cstate: ConversationStates,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().context("failed to read env config")?;
    setup_tracing().context("failed to setup tracing")?;

    let token = var("TOKEN").context("failed to read token from env")?;
    let bot = Bot::new(token);

    let pg_url = var("PG_URL").context("failed to read db connection info from env")?;
    let pg_conf = pg_url
        .parse::<PgConf>()
        .context("failed to parse db connection info")?;

    let (client, conn) = pg_conf
        .connect(NoTls)
        .await
        .context("failed to establish db connection")?;
    tokio::spawn(async {
        conn.await.expect("connection closed");
    });

    let ctx = ExecCtx {
        bot,
        db_client: client,
        cstate: ConversationStates::default(),
    };

    let mut interval = interval(Duration::from_millis(200));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut offset = 0;
    loop {
        interval.tick().await;

        let updates = ctx
            .bot
            .get_updates()
            .offset(offset)
            .limit(1)
            .allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::CallbackQuery])
            .await
            .context("failed to get updates")?;

        for update in updates {
            tracing::debug!("handling update");

            let Some(chat) = update.chat() else { continue; };
            let Some(user) = update.user() else { continue; };

            match update.kind {
                UpdateKind::Message(ref msg) => handle_message((chat, user), msg, &ctx)
                    .await
                    .context("failed to handle message")?,
                UpdateKind::CallbackQuery(ref cb) => handle_callback((chat, user), cb, &ctx)
                    .await
                    .context("failed to handle callback")?,
                _ => unreachable!(),
            }

            if update.id >= offset {
                offset = update.id + 1;
            }
        }
    }
}

async fn handle_message(
    (chat, user): (&Chat, &User),
    msg: &Message,
    ctx: &ExecCtx,
) -> anyhow::Result<()> {
    storage::user::ensure_exists(models::User { id: user.id.0 }, &ctx.db_client)
        .await
        .context("failed to ensure that user exists")?;

    match msg.text() {
        Some("/report") => {
            handlers::message::report(&ctx, chat.id, Utc::now().year()).await?;
        }
        Some("/add_expense") => {
            handlers::message::add_expense(&ctx, chat.id).await?;
        }
        Some(_text) => match ctx.cstate.get(user.id).await {
            Some(ConversationState::AwaitingCategoryName) => {
                handlers::message::category_name(&ctx, (user.id, chat.id), &msg).await?;
            }
            Some(ConversationState::AwaitingExpenseAmount { category_name }) => {
                handlers::message::expense_amount(&ctx, (user.id, chat.id), &msg, &category_name)
                    .await?;
            }
            _ => {
                ctx.bot
                    .delete_message(chat.id, msg.id)
                    .await
                    .context("failed to delete message")?;
            }
        },
        None => {
            tracing::warn!("empty message received");
        }
    }

    Ok(())
}

async fn handle_callback(
    (chat, user): (&Chat, &User),
    cb: &CallbackQuery,
    ctx: &ExecCtx,
) -> anyhow::Result<()> {
    let Some(cmd) = cb.data.as_ref() else { return Ok(()); };
    let cmd = cmd.parse::<Command>().context("failed to parse command")?;

    match cmd {
        Command::AddCategory => {
            ctx.bot
                .send_message(chat.id, "please, provide category name")
                .await
                .context("failed to send message")?;
            ctx.cstate
                .set(user.id, ConversationState::AwaitingCategoryName)
                .await;
        }
        Command::ConfirmCategoryName { msg_id } => {
            if let Some(ConversationState::AwaitingCategoryNameConfirmation {
                msg_id: expected_msg_id,
                category_name: cname,
            }) = ctx.cstate.get(user.id).await
            {
                if msg_id == expected_msg_id {
                    let inserted = storage::user::add_category(
                        models::User { id: user.id.0 },
                        &cname,
                        &ctx.db_client,
                    )
                    .await
                    .context("failed to add category")?;

                    let mut resp = inserted
                        .then(|| format!("category '{cname}' added"))
                        .unwrap_or_else(|| format!("category '{cname}' has already been added"));

                    resp.push_str("\n\nplease, provide expense amount");

                    ctx.bot
                        .send_message(chat.id, resp)
                        .await
                        .context("failed to send message")?;

                    ctx.cstate
                        .set(
                            user.id,
                            ConversationState::AwaitingExpenseAmount {
                                category_name: cname.to_string(),
                            },
                        )
                        .await;
                }
            }
        }
    }
    Ok(())
}

fn setup_tracing() -> anyhow::Result<()> {
    use std::io::{stderr, IsTerminal};

    use time::macros::format_description;
    use tracing_subscriber::{
        filter::{EnvFilter, LevelFilter},
        fmt::time::UtcTime,
    };

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .context("failed to create env filter")?;

    tracing_subscriber::fmt()
        .with_ansi(stderr().is_terminal())
        .with_writer(stderr)
        .with_target(false)
        .with_timer(UtcTime::new(format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"
        )))
        .with_env_filter(env_filter)
        .init();

    Ok(())
}
