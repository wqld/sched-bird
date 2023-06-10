mod auth;
mod db;
mod gpt;
mod render;
mod sched;
mod user;

use crate::user::User;

use std::collections::HashMap;
use std::convert::Infallible;
use std::env;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use axum::body::{boxed, Body, StreamBody};
use axum::error_handling::HandleError;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{middleware, Extension};
use axum::{routing::get, routing::post, Json, Router};
use clap::Parser;
use futures::stream::{self, StreamExt};
use hyper::server::Server;
use oauth2::basic::BasicClient;
use oauth2::{CsrfToken, Scope};
use sched_bird::{ServerApp, ServerAppProps};
use scylla::IntoTypedRows;
use serde::Deserialize;
use tower::ServiceExt;
use tower_cookies::{CookieManagerLayer, Cookies};
use tower_http::services::ServeDir;
use url::Url;
use yew::platform::Runtime;

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

#[derive(Clone, Default)]
struct Executor {
    inner: Runtime,
}

impl<F> hyper::rt::Executor<F> for Executor
where
    F: Future + Send + 'static,
{
    fn execute(&self, fut: F) {
        self.inner.spawn_pinned(move || async move {
            fut.await;
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let exec = Executor::default();

    let opt = Opt::parse();

    let sock_addr = SocketAddr::from((
        IpAddr::from_str(opt.addr.as_str()).unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
        opt.port,
    ));

    println!("Listening on {}", sock_addr);

    let client = auth::create_github_client();

    let (authorize_url, _csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user".to_string()))
        .url();

    println!("Browse to: {}", authorize_url);

    let db = Arc::new(db::Scylla::new().await?);

    let shared_state = Arc::new(AppState {
        db,
        client,
        authorize_url,
    });

    if let Some(rows) = shared_state
        .db
        .session
        .query("SELECT id, channel FROM ks.u", &[])
        .await?
        .rows
    {
        for row in rows.into_typed::<User>() {
            let row = row?;
            println!("row: {:?}", row);
        }
    }

    let index_path = PathBuf::from(&opt.dist).join("index.html");
    let index_html_s = tokio::fs::read_to_string(index_path)
        .await
        .expect("index.html not found");

    let (index_html_before, index_html_after) = index_html_s.split_once("<body>").unwrap();
    let mut index_html_before = index_html_before.to_owned();

    let head_start_index = index_html_before
        .find("<head>")
        .unwrap_or(index_html_before.len());

    let meta_viewport =
        r#"<meta name="viewport" content="width=device-width, initial-scale=1.0" />"#;
    index_html_before.insert_str(head_start_index, meta_viewport);

    let head_end_index = index_html_before
        .find("</head>")
        .unwrap_or(index_html_before.len());

    let tailwind_css = r#"<script src="https://cdn.tailwindcss.com"></script>"#;
    index_html_before.insert_str(head_end_index, tailwind_css);
    index_html_before.push_str("<body>");

    let index_html_after = index_html_after.to_owned();

    let handle_error = |e| async move {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Unhandled internal error: {e}"),
        )
    };

    let app = Router::new()
        .route("/auth", get(auth))
        .route("/api/v1/channels/:channel/scheds", get(get_scheds))
        .route("/api/v1/gpt", post(invoke_gpt))
        .with_state(Arc::clone(&shared_state))
        .route_layer(middleware::from_fn_with_state(shared_state, auth::auth))
        .fallback_service(HandleError::new(
            ServeDir::new(PathBuf::from(&opt.dist))
                .append_index_html_on_directories(false)
                .fallback(
                    get(render)
                        .with_state((index_html_before.clone(), index_html_after.clone()))
                        .map_err(|err| -> std::io::Error { match err {} }),
                ),
            handle_error,
        ))
        .layer(CookieManagerLayer::new());

    Server::bind(&sock_addr)
        .executor(exec)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

fn get_cookie_value(cookies: &Cookies, name: &str) -> String {
    cookies
        .get(name)
        .and_then(|c| c.value().parse().ok())
        .unwrap_or_default()
}

async fn render(
    url: Uri,
    cookies: Cookies,
    Query(queries): Query<HashMap<String, String>>,
    State((index_html_before, index_html_after)): State<(String, String)>,
) -> impl IntoResponse {
    let url = url.to_string();

    println!("cookies: {:?}", cookies);

    let user = get_cookie_value(&cookies, "user");
    let channel = get_cookie_value(&cookies, "channel");
    let token: String = get_cookie_value(&cookies, "auth_token");

    let renderer = yew::ServerRenderer::<ServerApp>::with_props(move || ServerAppProps {
        url: url.into(),
        queries,
        user,
        channel,
        token,
    });

    StreamBody::new(
        stream::once(async move { index_html_before })
            .chain(renderer.render_stream())
            .chain(stream::once(async move { index_html_after }))
            .map(Result::<_, Infallible>::Ok),
    )
}

async fn auth() -> impl IntoResponse {
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, "https://sched.sinabro.io/")
        .body(boxed(Body::empty()))
        .unwrap()
}

async fn get_scheds(
    Path(channel): Path<String>,
    Extension(user): Extension<User>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let scheds = state.db.find_sched_by_channel(&channel).await.unwrap();

    let content = serde_json::json!({ "user": user.id, "channel": user.channel, "data": scheds });

    println!("scheds: {:?}", content);

    Json(content)
}

#[derive(Deserialize, Debug)]
struct OpenAiRequest {
    query: String,
}

async fn invoke_gpt(
    Extension(user): Extension<User>,
    State(state): State<Arc<AppState>>,
    Json(input): Json<OpenAiRequest>,
) -> impl IntoResponse {
    let open_ai_secret = env::var("OPENAI_SECRET").unwrap();

    println!("input: {:?}", input);

    let query = gpt::request_gpt_api(&open_ai_secret, &input.query).await;

    println!("query: {:?}", query);

    match query {
        Ok(query) => match state.db.insert(&query).await {
            Ok(_) => {
                let scheds = state.db.find_sched_by_channel(&user.channel).await.unwrap();
                let content =
                    serde_json::json!({ "user": user.id, "channel": user.channel, "data": scheds });
                return Response::builder()
                    .status(StatusCode::OK)
                    .body(boxed(Body::from(content.to_string())))
                    .unwrap();
            }
            Err(err) => {
                println!("err: {:?}", err);
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(boxed(Body::from(err.to_string())))
                    .unwrap()
            }
        },
        Err(err) => {
            println!("err: {:?}", err);
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(boxed(Body::from(err.to_string())))
                .unwrap()
        }
    }
}
