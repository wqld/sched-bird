use std::env;

use anyhow::Result;
use axum::{routing::get, Router};
use scylla::macros::FromRow;
use scylla::{IntoTypedRows, Session, SessionBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    let uri = env::var("SCYLLA_URI").unwrap_or_else(|_| "db:9042".to_string());

    println!("Connecting at {}", uri);

    let session: Session = SessionBuilder::new().known_node(uri).build().await?;

    session.query("CREATE KEYSPACE IF NOT EXISTS ks WITH REPLICATION = {'class' : 'SimpleStrategy', 'replication_factor' : 1}", &[]).await?;
    session
        .query(
            "CREATE TABLE IF NOT EXISTS ks.t (key text primary key, value text)",
            &[],
        )
        .await?;

    let prepared = session
        .prepare("INSERT INTO ks.t (key, value) VALUES (?, ?)")
        .await?;

    session.execute(&prepared, ("key", "value")).await?;

    #[derive(Debug, FromRow)]
    struct Row {
        key: String,
        value: String,
    }

    if let Some(rows) = session
        .query("SELECT key, value FROM ks.t", &[])
        .await?
        .rows
    {
        for row in rows.into_typed::<Row>() {
            let row = row?;
            println!("row: {:?}", row);
        }
    }

    let app = Router::new().route("/", get(|| async { "수지야 젤다 고고" }));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
