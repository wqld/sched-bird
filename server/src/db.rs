use std::env;

use anyhow::Result;
use scylla::{IntoTypedRows, Session, SessionBuilder};

use crate::user::User;

pub struct Scylla {
    pub session: Session,
}

impl Scylla {
    pub async fn new() -> Result<Self> {
        let uri = env::var("SCYLLA_URI").unwrap_or_else(|_| "db:9042".to_string());

        println!("Connecting at {}", uri);

        let session: Session = SessionBuilder::new().known_node(uri).build().await?;

        session.query("CREATE KEYSPACE IF NOT EXISTS ks WITH REPLICATION = {'class' : 'SimpleStrategy', 'replication_factor' : 1}", &[]).await?;

        session
            .query(
                "CREATE TABLE IF NOT EXISTS ks.u (id text primary key, group text, auth_token text)",
                &[],
            )
            .await?;

        let prepared = session
            .prepare("INSERT INTO ks.u (id, group, auth_token) VALUES (?, ?, ?)")
            .await?;

        session.execute(&prepared, ("21kyu", "home", "")).await?;
        session.execute(&prepared, ("csj20045", "home", "")).await?;

        Ok(Self { session })
    }

    pub async fn find_user_by_id(&self, id: &str) -> Result<Option<User>> {
        if let Some(rows) = self
            .session
            .query("SELECT id, group, auth_token FROM ks.u WHERE id = ?", (id,))
            .await?
            .rows
        {
            let row = rows.into_typed::<User>().next().unwrap()?;
            return Ok(Some(row));
        } else {
            Ok(None)
        }
    }

    pub async fn find_user_by_auth_token(&self, auth_token: &str) -> Result<Option<User>> {
        if let Some(rows) = self
            .session
            .query(
                "SELECT id, group, auth_token FROM ks.u WHERE auth_token = ?",
                (auth_token,),
            )
            .await?
            .rows
        {
            let row = rows.into_typed::<User>().next().unwrap()?;
            return Ok(Some(row));
        } else {
            Ok(None)
        }
    }

    pub async fn insert_user(&self, user: &User) -> Result<()> {
        let prepared = self
            .session
            .prepare("INSERT INTO ks.u (id, group, auth_token) VALUES (?, ?, ?)")
            .await?;

        self.session
            .execute(
                &prepared,
                (
                    user.id.as_str(),
                    user.group.as_str(),
                    user.auth_token.as_str(),
                ),
            )
            .await?;

        Ok(())
    }

    pub async fn update_user(&self, user: &User) -> Result<()> {
        let prepared = self
            .session
            .prepare("UPDATE ks.u SET group = ?, auth_token = ? WHERE id = ?")
            .await?;

        self.session
            .execute(
                &prepared,
                (
                    user.group.as_str(),
                    user.auth_token.as_str(),
                    user.id.as_str(),
                ),
            )
            .await?;

        Ok(())
    }
}
