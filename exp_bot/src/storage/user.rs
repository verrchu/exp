use crate::{models::User, PgClient};

use anyhow::Context;

pub async fn ensure_exists(user: User, db_client: &PgClient) -> anyhow::Result<()> {
    let stmt = db_client
        .prepare("insert into users(id) values($1) on conflict do nothing")
        .await
        .context("failed to prepare query")?;

    let user_id = i64::try_from(user.id).context("failed to cast user id to i64")?;
    db_client
        .execute(&stmt, &[&user_id])
        .await
        .context("failed to execute statement")?;

    Ok(())
}
