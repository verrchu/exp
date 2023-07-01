mod conversation_state;
use conversation_state::{ConversationState, ConversationStates};
mod command;
use command::Command;
mod handlers;
mod storage;
use storage::Storage;

use std::{env::var, time::Duration};

use anyhow::Context;

use teloxide_core::{
    payloads::GetUpdatesSetters,
    requests::Requester,
    types::{AllowedUpdate, Chat, Message, UpdateKind, User},
    Bot,
};
use tokio::time::{interval, MissedTickBehavior};
use tokio_postgres::{Client as PgClient, Config as PgConf, NoTls};

struct ExecCtx {
    bot: Bot,
    storage: Storage,
    cstate: ConversationStates,
}

struct MsgCtx {
    user: User,
    chat: Chat,
    msg: Message,
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

    let exec_ctx = ExecCtx {
        bot,
        storage: Storage::new(client),
        cstate: ConversationStates::default(),
    };

    let mut interval = interval(Duration::from_millis(200));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut offset = 0;
    loop {
        interval.tick().await;

        let updates = exec_ctx
            .bot
            .get_updates()
            .offset(offset)
            .limit(1)
            .allowed_updates(vec![AllowedUpdate::Message, AllowedUpdate::CallbackQuery])
            .await
            .context("failed to get updates")?;

        for update in updates {
            tracing::debug!("handling update");

            let Some(chat) = update.chat().cloned() else { continue; };
            let Some(user) = update.user().cloned() else { continue; };

            match update.kind {
                UpdateKind::Message(msg) => {
                    let msg_ctx = MsgCtx { user, chat, msg };
                    handle_message(&exec_ctx, &msg_ctx)
                        .await
                        .context("failed to handle message")?
                }
                UpdateKind::CallbackQuery(cb) => {
                    let Some(msg) = cb.message else { continue; };
                    let msg_ctx = MsgCtx { user, chat, msg };

                    let Some(cmd) = cb.data else { continue; };
                    let cmd = cmd
                        .parse::<Command>()
                        .context("failed to parse callback data")?;

                    handle_callback(cmd, &exec_ctx, &msg_ctx)
                        .await
                        .context("failed to handle callback")?
                }
                _ => unreachable!(),
            }

            if update.id >= offset {
                offset = update.id + 1;
            }
        }
    }
}

async fn handle_message(exec_ctx: &ExecCtx, msg_ctx: &MsgCtx) -> anyhow::Result<()> {
    exec_ctx
        .storage
        .ensure_exists(&msg_ctx.user)
        .await
        .context("failed to ensure that user exists")?;

    match msg_ctx.msg.text() {
        Some("/report") => {
            handlers::message::report(exec_ctx, msg_ctx).await?;
        }
        Some("/add_expense") => {
            handlers::message::add_expense(exec_ctx, msg_ctx).await?;
        }
        Some(_text) => match exec_ctx.cstate.get(msg_ctx.user.id).await {
            Some(ConversationState::AwaitingCategoryName) => {
                handlers::message::category_name(exec_ctx, msg_ctx).await?;
            }
            Some(ConversationState::AwaitingExpenseAmount {
                category_name,
                date,
            }) => {
                handlers::message::expense_amount(exec_ctx, msg_ctx, &category_name, date).await?;
            }
            _ => {
                exec_ctx
                    .bot
                    .delete_message(msg_ctx.chat.id, msg_ctx.msg.id)
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

async fn handle_callback(cmd: Command, exec_ctx: &ExecCtx, msg_ctx: &MsgCtx) -> anyhow::Result<()> {
    match cmd {
        Command::AddCategory => {
            handlers::callback::add_category(exec_ctx, msg_ctx).await?;
        }
        Command::ConfirmCategoryName {
            msg_id: source_msg_id,
        } => {
            handlers::callback::confirm_category_name(source_msg_id, exec_ctx, msg_ctx).await?;
        }
        Command::RejectCategoryName {
            msg_id: source_msg_id,
        } => {
            handlers::callback::reject_category_name(source_msg_id, exec_ctx, msg_ctx).await?;
        }
        Command::PickExpenseDate {
            msg_id: source_msg_id,
            date,
        } => {
            handlers::callback::pick_expense_date(source_msg_id, date, exec_ctx, msg_ctx).await?;
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
