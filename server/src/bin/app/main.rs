mod auth;
mod db;
mod render;
mod sched;
mod user;

use crate::user::User;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use axum::body::{boxed, Body};
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::{middleware, Extension};
use axum::{routing::get, Router};
use clap::Parser;
use oauth2::basic::BasicClient;
use oauth2::{CsrfToken, Scope};
use render::render_app;
use sched_bird::App;
use scylla::IntoTypedRows;
use tokio::fs;
use tower::util::ServiceExt;
use tower_cookies::CookieManagerLayer;
use tower_http::services::ServeDir;
use url::Url;

#[derive(Parser, Debug)]
#[clap(name = "Sched Bird")]
struct Opt {
    #[clap(short = 'a', long = "addr", default_value = "0.0.0.0")]
    addr: String,

    #[clap(short = 'p', long = "port", default_value = "3000")]
    port: u16,

    #[clap(short = 'd', long = "dist", default_value = "../../../dist")]
    dist: String,
}

#[derive(Clone)]
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
        .route("/", get(render))
        .route("/auth", get(auth))
        .route("/api/v1/groups/:group_id/scheds", get(handler))
        .with_state(Arc::clone(&shared_state))
        .route_layer(middleware::from_fn_with_state(shared_state, auth::auth))
        .layer(CookieManagerLayer::new())
        .fallback(get(|req| async move {
            match ServeDir::new(&opt.dist).oneshot(req).await {
                Ok(res) => {
                    let status = res.status();
                    match status {
                        StatusCode::NOT_FOUND => {
                            let index_path = PathBuf::from(&opt.dist).join("index.html");
                            let index_content = match fs::read_to_string(index_path).await {
                                Err(_) => {
                                    return Response::builder()
                                        .status(StatusCode::NOT_FOUND)
                                        .body(boxed(Body::from("index file not found")))
                                        .unwrap()
                                }
                                Ok(index_content) => index_content,
                            };

                            let (index_html_before, index_html_after) =
                                index_content.split_once("<body>").unwrap();
                            let index_html_before = format!("{}<body>", index_html_before);

                            let renderer = yew::ServerRenderer::<App>::new();
                            let rendered = renderer.render().await;
                            let html =
                                format!("{}{}{}", index_html_before, rendered, index_html_after);

                            Html(html).into_response()
                        }
                        _ => res.map(boxed),
                    }
                }
                Err(err) => Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(boxed(Body::from(format!("error: {err}"))))
                    .expect("error response"),
            }
        }));

    axum::Server::bind(&sock_addr)
        .serve(app.into_make_service())
        .await
        .expect("server failed to start");

    Ok(())
}

async fn render() -> impl IntoResponse {
    let index_path = PathBuf::from("../../../dist").join("index.html");
    let index_content = match fs::read_to_string(index_path).await {
        Err(_) => {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(boxed(Body::from("index file not found")))
                .unwrap()
        }
        Ok(index_content) => index_content,
    };

    let (index_html_before, index_html_after) = index_content.split_once("<body>").unwrap();
    let index_html_before = format!("{}<body>", index_html_before);

    let renderer = yew::ServerRenderer::<App>::new();
    let rendered = renderer.render().await;
    let html = format!("{}{}{}", index_html_before, rendered, index_html_after);

    println!("html: {}", html);

    Html(html).into_response()
}

async fn root(
    Extension(user): Extension<User>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let id = user.id.clone();

    let scheds = state.db.find_sched_by_group(&user.group).await.unwrap();

    println!("scheds: {:?}", scheds);

    let content = render_app(id, scheds).await;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .header(header::AUTHORIZATION, user.auth_token)
        .body(boxed(Body::from(content)))
        .unwrap()
}

async fn auth() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "https://sched.sinabro.io/")
        .body(boxed(Body::empty()))
        .unwrap()
}

async fn handler(
    Path(group_id): Path<String>,
    Extension(user): Extension<User>,
) -> impl IntoResponse {
    format!("Hello, {}/{}!", user.id, group_id)
}
