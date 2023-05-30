use std::env;
use std::sync::Arc;

use anyhow::Result;
use axum::body::{boxed, Body};
use axum::extract::State;
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, TokenResponse,
    TokenUrl,
};
use octocrab::Octocrab;
use tower_cookies::{Cookie, Cookies};

use crate::user::User;
use crate::AppState;

pub async fn auth<B>(
    cookies: Cookies,
    State(shared): State<Arc<AppState>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let cookie_token: Option<String> = cookies
        .get("auth_token")
        .and_then(|c| c.value().parse().ok());

    match cookie_token {
        Some(token) => {
            println!("token: {}", token);

            let user_opt = shared.db.find_user_by_auth_token(&token).await.unwrap();

            match user_opt {
                Some(user) => {
                    println!("user: {:?}", user);
                    return Ok(auth_next(req, next, user).await);
                }
                None => {
                    cookies.remove(Cookie::new("auth_token", ""));
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
        }
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

pub fn create_github_client() -> BasicClient {
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
        RedirectUrl::new("https://sched.sinabro.io/auth".to_string())
            .expect("Invalid redirect url"),
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

    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(format!("auth_token={}", token).as_str()).unwrap(),
    );

    res
}
