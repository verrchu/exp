use crate::{models::User, PgClient};

use anyhow::Context;
use rust_decimal::Decimal;

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

pub async fn add_category(user: User, cname: &str, db_client: &PgClient) -> anyhow::Result<bool> {
    let stmt = db_client
        .prepare("insert into categories(user_id, category) values($1, $2) on conflict do nothing")
        .await
        .context("failed to prepare query")?;

    let user_id = i64::try_from(user.id).context("failed to cast user id to i64")?;
    let nmod = db_client
        .execute(&stmt, &[&user_id, &cname])
        .await
        .context("failed to execute statement")?;

    Ok(nmod > 0)
}

pub async fn add_expense(
    user: User,
    cname: &str,
    amount: Decimal,
    db_client: &PgClient,
) -> anyhow::Result<bool> {
    let stmt = db_client
        .prepare(
            "insert into expenses(user_id, category_id, amount)
             values($1, (select id from categories where category = $2), $3)",
        )
        .await
        .context("failed to prepare query")?;

    let user_id = i64::try_from(user.id).context("failed to cast user id to i64")?;
    let nmod = db_client
        .execute(&stmt, &[&user_id, &cname, &amount])
        .await
        .context("failed to execute statement")?;

    Ok(nmod > 0)
}
