mod db;
mod render;
mod user;

use crate::user::User;

use std::env;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Result;
use axum::body::{boxed, Body};
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{middleware, Extension};
use axum::{routing::get, Router};
use clap::Parser;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use octocrab::Octocrab;
use render::render_app;
use scylla::IntoTypedRows;
use tokio::task::{spawn_blocking, LocalSet};
use url::Url;

#[derive(Parser, Debug)]
#[clap(name = "Sched Bird")]
struct Opt {
    #[clap(short = 'a', long = "addr", default_value = "0.0.0.0")]
    addr: String,

    #[clap(short = 'p', long = "port", default_value = "3000")]
    port: u16,
}

struct AppState {
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

    let client = create_github_client();

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
        .route_layer(middleware::from_fn_with_state(shared_state, auth));

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

async fn auth<B>(
    State(shared): State<Arc<AppState>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    match req.headers().get(header::AUTHORIZATION) {
        Some(header) => match header.to_str() {
            Ok(header) => {
                println!("header: {}", header);

                let user_opt = shared.db.find_user_by_auth_token(header).await.unwrap();

                match user_opt {
                    Some(user) => {
                        println!("user: {:?}", user);
                        return Ok(auth_next(req, next, user).await);
                    }
                    None => return Err(StatusCode::UNAUTHORIZED),
                }
            }
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        },
        None => {
            let query = req.uri().query().unwrap_or_default();

            match get_github_user_id_and_token(query, &shared).await {
                Ok((id, token)) => {
                    let user_opt = shared.db.find_user_by_id(&id).await.unwrap();
                    let user = match user_opt {
                        Some(mut user) => {
                            user.auth_token = token;
                            shared.db.update_user(&user).await.unwrap();
                            user
                        }
                        None => {
                            let user = User {
                                id,
                                group: "home".to_owned(),
                                auth_token: token,
                            };
                            shared.db.insert_user(&user).await.unwrap();
                            user
                        }
                    };

                    println!("user: {:?}", user);
                    return Ok(auth_next(req, next, user).await);
                }
                Err(StatusCode::FOUND) => {
                    let res = Response::builder()
                        .status(StatusCode::FOUND)
                        .header(header::LOCATION, shared.authorize_url.to_string())
                        .body(boxed(Body::empty()))
                        .unwrap();

                    return Ok(res);
                }
                _ => return Err(StatusCode::UNAUTHORIZED),
            }
        }
    }
}

fn create_github_client() -> BasicClient {
    let github_client_id =
        ClientId::new(env::var("GITHUB_CLIENT_ID").expect("Missing the GITHUB_CLIENT_ID env"));
    let github_client_secret = ClientSecret::new(
        env::var("GITHUB_CLIENT_SECRET").expect("Missing the GITHUB_CLIENT_SECRET env"),
    );

    let auth_url = AuthUrl::new("https://github.com/login/oauth/authorize".to_string())
        .expect("Invalid auth url");
    let token_url = TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
        .expect("Invalid token url");

    BasicClient::new(
        github_client_id,
        Some(github_client_secret),
        auth_url,
        Some(token_url),
    )
    .set_redirect_uri(
        RedirectUrl::new("https://sched.sinabro.io/".to_string()).expect("Invalid redirect url"),
    )
}

async fn get_github_user_id_and_token(
    query: &str,
    shared: &Arc<AppState>,
) -> Result<(String, String), StatusCode> {
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
        return Err(StatusCode::FOUND);
    }

    let code = AuthorizationCode::new(code.unwrap().to_string());
    let _state = CsrfToken::new(state.unwrap().to_string());

    println!("Github returned the following code:\n{}\n", code.secret());

    let token_res = shared
        .client
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

        println!("user id: {:?}", cur_user.login);

        return Ok((cur_user.login, token.access_token().secret().to_owned()));
    }

    Err(StatusCode::UNAUTHORIZED)
}

async fn auth_next<B>(mut req: Request<B>, next: Next<B>, user: User) -> Response {
    let token = user.auth_token.clone();

    req.extensions_mut().insert(user);

    let mut res = next.run(req).await;

    res.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(token.as_str()).unwrap(),
    );

    res
}
