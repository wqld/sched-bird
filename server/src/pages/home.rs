use serde::{Deserialize, Serialize};
use yew::prelude::*;

use crate::Auth;

#[derive(Serialize, Deserialize, Debug, Default)]
struct Sched {
    group: String,
    id: String,
    sched: String,
    date_at: String,
    create_at: String,
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct SchedResponse {
    data: Vec<Sched>,
}

#[cfg(feature = "ssr")]
async fn fetch_sched(token: String) -> SchedResponse {
    // reqwest works for both non-wasm and wasm targets.
    let client = reqwest::Client::new();
    let resp = client
        .get("https://sched.sinabro.io/api/v1/groups/home/scheds")
        .header("authorization", format!("Bearer {}", token))
        .header("group", "home")
        .send()
        .await
        .unwrap();

    if resp.status() != 200 {
        return SchedResponse::default();
    }

    let scheds = resp.json::<SchedResponse>().await;

    println!("scheds: {:?}", scheds);

    match scheds {
        Ok(scheds) => scheds,
        Err(_) => SchedResponse::default(),
    }
}

// #[cfg(feature = "ssr")]
// async fn fetch_token() -> String {
//     println!("fetch_token");

//     let resp = reqwest::get("https://sched.sinabro.io/auth/token")
//         .await
//         .unwrap();

//     let token = resp.headers().get("authorization");

//     match token {
//         Some(token) => token.to_str().unwrap().to_owned(),
//         None => "".to_owned(),
//     }
// }

#[function_component]
fn Content() -> HtmlResult {
    let ctx = use_context::<Auth>().expect("no ctx found");
    println!("ctx: {:?}", ctx);

    let token = ctx.token;

    let scheds = use_prepared_state!(
        async move |token| -> SchedResponse { fetch_sched(token.to_string()).await },
        token
    )?
    .unwrap();

    Ok(html! {
        <div>
            <h1>{"Home"}</h1>
            <ul>
                {for scheds.data.iter().map(|sched| {
                    html! {
                        <li>
                            {format!("{} | {} | {}", sched.id, sched.sched, sched.date_at)}
                        </li>
                    }
                })}
            </ul>
        </div>
    })
}

#[function_component]
pub fn Home() -> Html {
    let ctx = use_context::<Auth>().expect("no ctx found");
    let fallback = html! {<div>{"Loading..."}</div>};

    println!("ctx: {:?}", ctx);

    html! {
        <Suspense fallback={fallback}>
            { match ctx.token.is_empty() {
                true => html! {<div><a href="https://sched.sinabro.io/auth">{"Login with Github"}</a></div>},
                false => html! {<Content />},
            }}
        </Suspense>
    }
}
