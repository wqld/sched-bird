mod db;

use std::env;

use anyhow::Result;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{middleware, Extension, Json};
use axum::{routing::get, Router};
use http_body::combinators::UnsyncBoxBody;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use octocrab::Octocrab;
use scylla::macros::FromRow;
use scylla::{IntoTypedRows, Session, SessionBuilder};

#[derive(Debug, Clone)]
struct User {
    name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    openssl_probe::init_ssl_cert_env_vars();

    // db Connecting
    let db = db::Scylla::new().await?;

    #[derive(Debug, FromRow)]
    struct Row {
        id: String,
        group: String,
    }

    if let Some(rows) = db
        .session
        .query("SELECT id, group FROM ks.u", &[])
        .await?
        .rows
    {
        for row in rows.into_typed::<Row>() {
            let row = row?;
            println!("row: {:?}", row);
        }
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
            RedirectUrl::new("https://sched.sinabro.io/".to_string())
                .expect("Invalid redirect url"),
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
                name: cur_user.login,
            };

            req.extensions_mut().insert(user);
            return Ok(next.run(req).await);
        }

        Err(StatusCode::UNAUTHORIZED)
    }

    async fn handler(Extension(user): Extension<User>) -> impl IntoResponse {
        format!("Hello, {}!", user.name)
    }

    let app = Router::new()
        .route("/", get(handler))
        .route("/callback", get(|| async { "콜백이야" }))
        .route_layer(middleware::from_fn(auth));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
