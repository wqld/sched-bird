use std::env;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use axum::body::{boxed, Body};
use axum::extract::State;
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use hyper::HeaderMap;
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, RedirectUrl, TokenResponse, TokenUrl,
};
use octocrab::Octocrab;
use serde::{Deserialize, Serialize};
use tower_cookies::{Cookie, Cookies};

use crate::user::User;
use crate::AppState;

const BEARER: &str = "Bearer ";
const JWT_MAX_AGES: i64 = 60;
const DEFAULT_GROUP: &str = "home";

#[derive(Debug, Deserialize, Serialize)]
struct Claims {
    user: String,
    group: String,
    token: String,
    exp: usize,
}

pub fn create_jwt(user: &str, group: &str, token: &str) -> Result<String> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(JWT_MAX_AGES))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        user: user.to_owned(),
        group: group.to_owned(),
        token: token.to_owned(),
        exp: expiration as usize,
    };

    let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS512);
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(env::var("JWT_SECRET")?.as_bytes()),
    )
    .map_err(|e| anyhow!("failed to encode jwt: {}", e))
}

fn group_from_header(headers: &HeaderMap<HeaderValue>) -> Result<String> {
    let header = match headers.get("group") {
        Some(header) => header,
        None => return Err(anyhow!("missing group header")),
    };
    let group = match header.to_str() {
        Ok(group) => group,
        Err(_) => return Err(anyhow!("invalid group header")),
    };
    Ok(group.to_owned())
}

fn jwt_from_header(headers: &HeaderMap<HeaderValue>) -> Result<String> {
    let header = match headers.get(header::AUTHORIZATION) {
        Some(header) => header,
        None => return Err(anyhow!("missing authorization header")),
    };
    let auth_header = match header.to_str() {
        Ok(auth_header) => auth_header,
        Err(_) => return Err(anyhow!("invalid authorization header")),
    };
    if !auth_header.starts_with(BEARER) {
        return Err(anyhow!("invalid authorization header"));
    }
    Ok(auth_header[BEARER.len()..].to_owned())
}

fn jwt_from_cookie(cookies: &Cookies) -> Result<String> {
    let cookie = match cookies.get("auth_token") {
        Some(cookie) => cookie,
        None => return Err(anyhow!("missing auth_token cookie")),
    };
    let auth_cookie = match cookie.value().parse::<String>() {
        Ok(auth_cookie) => auth_cookie,
        Err(_) => return Err(anyhow!("invalid auth_token cookie")),
    };
    Ok(auth_cookie)
}

async fn authorize(group: &str, jwt: &str) -> Result<User> {
    let decoded = jsonwebtoken::decode::<Claims>(
        jwt,
        &jsonwebtoken::DecodingKey::from_secret(env::var("JWT_SECRET")?.as_bytes()),
        &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS512),
    )
    .map_err(|e| anyhow!("failed to decode jwt: {}", e))?;

    if decoded.claims.group != group {
        return Err(anyhow!("invalid group"));
    }

    if decoded.claims.exp < chrono::Utc::now().timestamp() as usize {
        return Err(anyhow!("expired jwt"));
    }

    Ok(User {
        id: decoded.claims.user,
        group: decoded.claims.group,
        auth_token: decoded.claims.token,
    })
}

pub async fn auth<B>(
    cookies: Cookies,
    State(shared): State<Arc<AppState>>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    println!("cookies len: {}", cookies.list().len());
    println!("request headers: {:?}", req.headers());

    let request_group = match group_from_header(req.headers()) {
        Ok(group) => group,
        _ => DEFAULT_GROUP.to_owned(),
    };

    let jwt = match jwt_from_header(req.headers()) {
        Ok(jwt) => Ok(jwt),
        _ => match jwt_from_cookie(&cookies) {
            Ok(jwt) => Ok(jwt),
            _ => Err(anyhow!("invalid jwt")),
        },
    };

    match jwt {
        Ok(jwt) => {
            println!("jwt: {:?}", jwt);
            let user = match authorize(&request_group, &jwt).await {
                Ok(user) => user,
                _ => return Err(StatusCode::UNAUTHORIZED),
            };
            println!("user: {:?}", user);
            Ok(auth_next(req, next, user, &cookies).await)
        }
        _ => {
            let query = req.uri().query().unwrap_or_default();
            println!("query: {}", query);

            match get_github_user_id_and_token(query, &shared).await {
                Ok((id, token)) => {
                    let user_opt = shared.db.find_user_by_id(&id).await.unwrap();
                    let jwt = create_jwt(&id, &request_group, &token).unwrap();
                    let user = match user_opt {
                        Some(mut user) => {
                            user.auth_token = jwt;
                            user
                        }
                        None => {
                            let user = User {
                                id,
                                group: request_group.clone(),
                                auth_token: jwt,
                            };
                            shared.db.insert_user(&user).await.unwrap();
                            user
                        }
                    };

                    println!("user: {:?}", user);
                    Ok(auth_next(req, next, user, &cookies).await)
                }
                Err(StatusCode::FOUND) => Ok(response_redirect_auth(&shared)),
                _ => Err(StatusCode::UNAUTHORIZED),
            }
        }
    }
}

fn response_redirect_auth(shared: &Arc<AppState>) -> Response {
    println!("redirect to github");

    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, shared.authorize_url.to_string())
        .body(boxed(Body::empty()))
        .unwrap()
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

    let code = AuthorizationCode::new(code.unwrap());
    // let _state = CsrfToken::new(state.unwrap().to_string());

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
                .flat_map(|comma_separated| comma_separated.split(','))
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

async fn auth_next<B>(
    mut req: Request<B>,
    next: Next<B>,
    user: User,
    cookies: &Cookies,
) -> Response {
    let token = user.auth_token.clone();
    let name = user.id.clone();

    println!("auth_next: {:?}", user);

    req.extensions_mut().insert(user);

    let mut res = next.run(req).await;

    res.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(token.as_str()).unwrap(),
    );

    let cookie_opts = format!(
        "Secure; HttpOnly; SameSite=None; Path=/; Max-Age={}",
        JWT_MAX_AGES
    );

    cookies.add(Cookie::parse(format!("user={}; {}", name, cookie_opts)).unwrap());
    cookies.add(Cookie::parse(format!("auth_token={}; {}", token, cookie_opts)).unwrap());

    res
}
