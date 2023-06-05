use serde::{Deserialize, Serialize};
use yew::prelude::*;

use crate::Auth;

#[derive(Serialize, Deserialize, Debug, Default)]
struct Sched {
    channel: String,
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
async fn fetch_sched(token: &str, channel: &str) -> SchedResponse {
    let client = reqwest::Client::new();
    let url = format!(
        "https://sched.sinabro.io/api/v1/channels/{}/scheds",
        channel
    );
    let resp = client
        .get(url)
        .header("authorization", format!("Bearer {}", token))
        .header("channel", "home")
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

#[function_component]
fn Content() -> HtmlResult {
    let ctx = use_context::<Auth>().unwrap();
    println!("ctx: {:?}", ctx);

    let user = ctx.user;

    let scheds = use_prepared_state!(
        async move |_| -> SchedResponse { fetch_sched(&ctx.token, &ctx.channel).await },
        ()
    )?
    .unwrap();

    Ok(html! {
        <div>
            <h1>{"Hello "}{user}</h1>
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
    let ctx = use_context::<Auth>().unwrap();
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
