mod models;
mod storage;

use std::{env::var, time::Duration};

use anyhow::Context;
use chrono::{Datelike, Utc};
use teloxide_core::{
    payloads::{GetUpdatesSetters, SendMessageSetters},
    requests::Requester,
    types::{
        AllowedUpdate, CallbackQuery, Chat, ChatId, InlineKeyboardButton, InlineKeyboardButtonKind,
        InlineKeyboardMarkup, Message, UpdateKind, User,
    },
    Bot,
};
use tokio::time::{interval, MissedTickBehavior};
use tokio_postgres::{Client as PgClient, Config as PgConf, NoTls};

struct ExecCtx {
    bot: Bot,
    db_client: PgClient,
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
                UpdateKind::CallbackQuery(cb) => handle_callback(cb, &ctx)
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
            render_report_menu(&ctx.bot, chat.id, Utc::now().year())
                .await
                .context("failed to render report menu")?;
        }
        _ => {
            ctx.bot
                .delete_message(chat.id, msg.id)
                .await
                .context("failed to delete message")?;
        }
    }

    Ok(())
}

async fn handle_callback(_cb: CallbackQuery, cts: &ExecCtx) -> anyhow::Result<()> {
    Ok(())
}

async fn render_report_menu(bot: &Bot, chat_id: ChatId, year: i32) -> anyhow::Result<()> {
    let make_row = |months: [&str; 4]| {
        months.map(|m| {
            InlineKeyboardButton::new(m, InlineKeyboardButtonKind::CallbackData(m.to_string()))
        })
    };

    bot.send_message(chat_id, format!("{year}"))
        .reply_markup(InlineKeyboardMarkup::new([
            make_row(["Jan", "Feb", "Mar", "Apr"]),
            make_row(["May", "Jun", "Jul", "Aug"]),
            make_row(["Sep", "Oct", "Nov", "Dec"]),
        ]))
        .await
        .context("failed to send message")?;

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
