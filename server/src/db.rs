use std::env;

use anyhow::Result;
use scylla::{Session, SessionBuilder};

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
                "CREATE TABLE IF NOT EXISTS ks.u (id text primary key, group text)",
                &[],
            )
            .await?;

        let prepared = session
            .prepare("INSERT INTO ks.u (id, group) VALUES (?, ?)")
            .await?;

        session.execute(&prepared, ("21kyu", "home")).await?;
        session.execute(&prepared, ("csj20045", "home")).await?;

        Ok(Self { session })
    }
}
