mod auth;
mod db;
mod render;
mod user;

use crate::user::User;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use axum::body::{boxed, Body};
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{middleware, Extension};
use axum::{routing::get, Router};
use clap::Parser;
use oauth2::basic::BasicClient;
use oauth2::{CsrfToken, Scope};
use render::render_app;
use scylla::IntoTypedRows;
use url::Url;

#[derive(Parser, Debug)]
#[clap(name = "Sched Bird")]
struct Opt {
    #[clap(short = 'a', long = "addr", default_value = "0.0.0.0")]
    addr: String,

    #[clap(short = 'p', long = "port", default_value = "3000")]
    port: u16,
}

pub struct AppState {
    db: Arc<db::Scylla>,
    client: BasicClient,
    authorize_url: Url,
}

#[tokio::main]
async fn main() -> Result<()> {
    openssl_probe::init_ssl_cert_env_vars();

    let opt = Opt::parse();

    let sock_addr = SocketAddr::from((
        IpAddr::from_str(opt.addr.as_str()).unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        opt.port,
    ));

    println!("Listening on http://{}", sock_addr);

    let client = auth::create_github_client();

    let (authorize_url, _csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user".to_string()))
        .url();

    println!("Browse to: {}", authorize_url.to_string());

    let db = Arc::new(db::Scylla::new().await?);

    let shared_state = Arc::new(AppState {
        db,
        client,
        authorize_url,
    });

    if let Some(rows) = shared_state
        .db
        .session
        .query("SELECT id, group, auth_token FROM ks.u", &[])
        .await?
        .rows
    {
        for row in rows.into_typed::<User>() {
            let row = row?;
            println!("row: {:?}", row);
        }
    }

    let app = Router::new()
        .route("/", get(root))
        .route("/api/v1/groups/:group_id/scheds", get(handler))
        .with_state(Arc::clone(&shared_state))
        .route_layer(middleware::from_fn_with_state(shared_state, auth::auth));

    axum::Server::bind(&sock_addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed to start");

    Ok(())
}

async fn root(Extension(user): Extension<User>) -> impl IntoResponse {
    let id = user.id.clone();

    let content = render_app(id).await;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::AUTHORIZATION, user.auth_token)
        .body(boxed(Body::from(content)))
        .unwrap()
}

async fn handler(
    Path(group_id): Path<String>,
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    format!("Hello, {}/{}!", user.id, group_id)
}
