use std::env;

use anyhow::Result;
use scylla::{IntoTypedRows, Session, SessionBuilder};

use crate::{sched::Sched, user::User};

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
                "CREATE TABLE IF NOT EXISTS ks.u (id text primary key, channel text)",
                &[],
            )
            .await?;

        session
            .query("CREATE TABLE IF NOT EXISTS ks.s (channel text, id text, sched text, date_at date, create_at timestamp,
                PRIMARY KEY (channel, date_at, id, create_at))", &[])
            .await?;

        let prepared = session
            .prepare("INSERT INTO ks.u (id, channel) VALUES (?, ?)")
            .await?;

        session.execute(&prepared, ("21kyu", "home")).await?;
        session.execute(&prepared, ("csj200045", "home")).await?;

        // session.query("INSERT INTO ks.s (channel, id, sched, date_at, create_at) VALUES ('home', '21kyu', '봄소풍', '2023-06-26', toTimestamp(now()))", &[]).await?;
        // session.query("INSERT INTO ks.s (channel, id, sched, date_at, create_at) VALUES ('home', 'csj200045', 'I have to go play..', '2023-06-09', toTimestamp(now()))", &[]).await?;
        // session.query("INSERT INTO ks.s (channel, id, sched, date_at, create_at) VALUES ('home', 'csj200045', 'hello world~', '2023-06-30', toTimestamp(now()))", &[]).await?;

        Ok(Self { session })
    }

    pub async fn find_user_by_id(&self, id: &str) -> Result<Option<User>> {
        if let Some(rows) = self
            .session
            .query("SELECT id, channel FROM ks.u WHERE id = ?", (id,))
            .await?
            .rows
        {
            let row = rows.into_typed::<User>().next().unwrap()?;
            Ok(Some(row))
        } else {
            Ok(None)
        }
    }

    pub async fn insert_user(&self, user: &User) -> Result<()> {
        let prepared = self
            .session
            .prepare("INSERT INTO ks.u (id, channel) VALUES (?, ?)")
            .await?;

        self.session
            .execute(&prepared, (user.id.as_str(), user.channel.as_str()))
            .await?;

        Ok(())
    }

    pub async fn find_sched_by_channel(&self, channel: &str) -> Result<Vec<Sched>> {
        let q = "SELECT channel, id, sched, date_at, create_at FROM ks.s WHERE channel = ?";
        let prepared = self.session.prepare(q).await?;
        Ok(
            match self.session.execute(&prepared, (channel,)).await?.rows {
                Some(rows) => rows.into_typed::<Sched>().map(|s| s.unwrap()).collect(),
                _ => vec![],
            },
        )
    }

    pub async fn insert(&self, query: &str) -> Result<()> {
        println!("submitted query: {}", query);
        self.session.query(query, &[]).await?;
        Ok(())
    }
}
