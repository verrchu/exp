use crate::PgClient;

use anyhow::Context;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use teloxide_core::types::User;

pub struct Storage {
    client: PgClient,
}

impl Storage {
    pub fn new(client: PgClient) -> Self {
        Self { client }
    }

    pub async fn ensure_exists(&self, user: &User) -> anyhow::Result<()> {
        let stmt = self
            .client
            .prepare("insert into users(id) values($1) on conflict do nothing")
            .await
            .context("failed to prepare query")?;

        let user_id = i64::try_from(user.id.0).context("failed to cast user id to i64")?;
        self.client
            .execute(&stmt, &[&user_id])
            .await
            .context("failed to execute statement")?;

        Ok(())
    }

    pub async fn add_category(&self, user: &User, cname: &str) -> anyhow::Result<bool> {
        let stmt = self
            .client
            .prepare(
                "insert into categories(user_id, category) values($1, $2) on conflict do nothing",
            )
            .await
            .context("failed to prepare query")?;

        let user_id = i64::try_from(user.id.0).context("failed to cast user id to i64")?;
        let nmod = self
            .client
            .execute(&stmt, &[&user_id, &cname])
            .await
            .context("failed to execute statement")?;

        Ok(nmod > 0)
    }

    pub async fn add_expense(
        &self,
        user: &User,
        cname: &str,
        amount: Decimal,
        date: NaiveDate,
    ) -> anyhow::Result<bool> {
        let stmt = self
            .client
            .prepare(
                "insert into expenses(user_id, category_id, amount, date)
             values($1, (select id from categories where category = $2), $3, $4)",
            )
            .await
            .context("failed to prepare query")?;

        let user_id = i64::try_from(user.id.0).context("failed to cast user id to i64")?;
        let nmod = self
            .client
            .execute(&stmt, &[&user_id, &cname, &amount, &date])
            .await
            .context("failed to execute statement")?;

        Ok(nmod > 0)
    }
}
