use anyhow::{Context, Result};
use scylla::{
    client::session::Session, client::session_builder::SessionBuilder,
    statement::prepared::PreparedStatement,
};
use std::sync::Arc;

use crate::{driver::Driver, types::PublicKey};

const CREATE_KEYSPACE: &str = "
    CREATE KEYSPACE IF NOT EXISTS push_notifications
    WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1}
";

const CREATE_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS push_notifications.fcm_tokens (
        public_key  text PRIMARY KEY,
        fcm_token   text,
        updated_at  timestamp
    )
";

pub struct Cassandra {
    session: Arc<Session>,
    upsert_token: PreparedStatement,
    get_token: PreparedStatement,
}

impl Cassandra {
    pub async fn new(contact_point: &str) -> Result<Self> {
        let session: Session = SessionBuilder::new()
            .known_node(contact_point)
            .build()
            .await
            .context("failed to connect to Cassandra")?;

        session
            .query_unpaged(CREATE_KEYSPACE, &[])
            .await
            .context("failed to create keyspace")?;

        session
            .query_unpaged(CREATE_TABLE, &[])
            .await
            .context("failed to create table")?;

        let upsert_token = session
            .prepare(
                "INSERT INTO push_notifications.fcm_tokens \
                 (public_key, fcm_token, updated_at) \
                 VALUES (?, ?, toTimestamp(now()))",
            )
            .await
            .context("failed to prepare upsert statement")?;

        let get_token = session
            .prepare(
                "SELECT fcm_token FROM push_notifications.fcm_tokens \
                 WHERE public_key = ?",
            )
            .await
            .context("failed to prepare select statement")?;

        Ok(Self {
            session: Arc::new(session),
            upsert_token,
            get_token,
        })
    }
}

#[async_trait::async_trait]
impl Driver for Cassandra {
    async fn store_fcm_token(&self, public_key: &PublicKey, fcm_token: &str) -> Result<()> {
        self.session
            .execute_unpaged(&self.upsert_token, (public_key as &str, fcm_token))
            .await
            .context("failed to store FCM token")?;
        Ok(())
    }

    async fn get_fcm_token(&self, public_key: &PublicKey) -> Result<Option<String>> {
        let result = self
            .session
            .execute_unpaged(&self.get_token, (public_key as &str,))
            .await
            .context("failed to query FCM token")?;

        let rows = result.into_rows_result().context("failed to get rows")?;
        let first = rows.rows::<(String,)>()?.next();
        match first {
            Some(Ok((token,))) => Ok(Some(token)),
            Some(Err(e)) => Err(anyhow::anyhow!("{e}").context("failed to deserialize row")),
            None => Ok(None),
        }
    }
}
