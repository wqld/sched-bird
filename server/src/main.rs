mod db;

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::Result;
use axum::body::{boxed, Body};
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{middleware, Extension};
use axum::{routing::get, Router};
use clap::Parser;
use http_body::combinators::UnsyncBoxBody;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use octocrab::Octocrab;
use scylla::macros::FromRow;
use scylla::IntoTypedRows;
use tokio::fs;
use tower::ServiceExt;
use tower_http::services::ServeDir;

#[derive(Parser, Debug)]
#[clap(name = "Sched Bird")]
struct Opt {
    #[clap(short = 'a', long = "addr", default_value = "0.0.0.0")]
    addr: String,

    #[clap(short = 'p', long = "port", default_value = "3000")]
    port: u16,

    #[clap(short = 's', long = "static", default_value = "./dist")]
    static_dir: String,
}

#[derive(Debug, Default, Clone, FromRow)]
struct User {
    id: String,
    group: String,
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

    let db = db::Scylla::new().await?;

    if let Some(rows) = db
        .session
        .query("SELECT id, group FROM ks.u", &[])
        .await?
        .rows
    {
        for row in rows.into_typed::<User>() {
            let row = row?;
            println!("row: {:?}", row);
        }
    }

    let app = Router::new()
        // .route("/", get(handler))
        .route("/api/hello", get(hello))
        // .route_layer(middleware::from_fn(auth))
        .fallback(get(|req| async move {
            match ServeDir::new(&opt.static_dir).oneshot(req).await {
                Ok(res) => {
                    let status = res.status();
                    match status {
                        StatusCode::NOT_FOUND => {
                            let index_path = PathBuf::from(&opt.static_dir).join("index.html");
                            let index_content = match fs::read_to_string(index_path).await {
                                Err(_) => {
                                    return Response::builder()
                                        .status(StatusCode::NOT_FOUND)
                                        .body(boxed(Body::from("index file not found")))
                                        .unwrap()
                                }
                                Ok(index_content) => index_content,
                            };

                            Response::builder()
                                .status(StatusCode::OK)
                                .body(boxed(Body::from(index_content)))
                                .unwrap()
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

async fn hello() -> impl IntoResponse {
    "hello from server!"
}

async fn handler(Extension(user): Extension<User>) -> impl IntoResponse {
    format!("Hello, {}!", user.id)
}

async fn auth<B>(mut req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    let github_client_id =
        ClientId::new(env::var("GITHUB_CLIENT_ID").expect("Missing the GITHUB_CLIENT_ID env"));
    let github_client_secret = ClientSecret::new(
        env::var("GITHUB_CLIENT_SECRET").expect("Missing the GITHUB_CLIENT_SECRET env"),
    );

    let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
        .expect("Invalid auth url");
    let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
        .expect("Invalid token url");

    let client = BasicClient::new(
        github_client_id,
        Some(github_client_secret),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(
        RedirectUrl::new("https://sched.sinabro.io/".to_string()).expect("Invalid redirect url"),
    );

    let (authorize_url, csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("user".to_string()))
        .url();

    println!("Browse to: {}", authorize_url.to_string());

    let query = req.uri().query().unwrap_or_default();

    println!("query: {}", query);

    let mut params = url::form_urlencoded::parse(query.as_bytes()).into_owned();

    let code = params
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value);

    let state = params
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value);

    println!("code: {:?}, state: {:?}", code, state);

    if code.is_none() || state.is_none() {
        let res = Response::builder()
            .status(StatusCode::FOUND)
            .header(header::LOCATION, authorize_url.to_string())
            .body(UnsyncBoxBody::default())
            .unwrap();

        return Ok(res);
    }

    let code = AuthorizationCode::new(code.unwrap().to_string());
    let state = CsrfToken::new(state.unwrap().to_string());

    println!("Github returned the following code:\n{}\n", code.secret());
    println!(
        "Github returned the following state:\n{} (expected `{}`)\n",
        state.secret(),
        csrf_state.secret()
    );

    let token_res = client
        .exchange_code(code)
        .request_async(async_http_client)
        .await;

    println!("Github returned the following token:\n{:?}\n", token_res);

    if let Ok(token) = token_res {
        let scopes = if let Some(scopes_vec) = token.scopes() {
            scopes_vec
                .iter()
                .map(|comma_separated| comma_separated.split(','))
                .flatten()
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        println!("Github returned the following scopes:\n{:?}\n", scopes);

        let octocrab = Octocrab::builder()
            .personal_token(token.access_token().secret().clone())
            .build()
            .unwrap();

        let cur_user = octocrab.current().user().await.unwrap();

        println!("user: {:?}", cur_user.login);

        let user = User {
            id: cur_user.login,
            ..Default::default()
        };

        req.extensions_mut().insert(user);
        return Ok(next.run(req).await);
    }

    Err(StatusCode::UNAUTHORIZED)
}
