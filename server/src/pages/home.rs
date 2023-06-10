use serde::{Deserialize, Serialize};
use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::{
    components::{item::Item, login::Login},
    Auth,
};

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
pub struct Sched {
    channel: String,
    id: String,
    sched: String,
    date_at: String,
    create_at: String,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Clone)]
struct SchedResponse {
    user: String,
    channel: String,
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
        .header("channel", channel)
        .send()
        .await
        .unwrap();

    if resp.status() != 200 {
        return SchedResponse::default();
    }

    let scheds = resp.json::<SchedResponse>().await;

    match scheds {
        Ok(scheds) => scheds,
        Err(_) => SchedResponse::default(),
    }
}

#[function_component]
fn Content() -> HtmlResult {
    let ctx = use_context::<Auth>().unwrap();

    let scheds = use_prepared_state!(
        async move |_| -> SchedResponse { fetch_sched(&ctx.token, &ctx.channel).await },
        ()
    )?
    .unwrap();

    let message = use_state(|| "".to_string());
    let send = use_state(|| false);
    let state = use_state_eq(|| SchedResponse {
        user: scheds.user.to_string(),
        channel: scheds.channel.to_string(),
        data: scheds.data.to_owned(),
    });

    let onchange = {
        let message = message.clone();

        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            message.set(input.value());
        })
    };

    let onclick = {
        let message = message.clone();
        let send = send.clone();
        let state = state.clone();

        Callback::from(move |_| {
            send.set(true);
            let send = send.clone();
            let message = message.clone();
            let state = state.clone();
            let user = state.user.to_string();
            let channel = state.channel.to_string();

            #[cfg(feature = "hydration")]
            {
                let mut map = std::collections::HashMap::new();
                map.insert(
                    "query",
                    format!("{} (channel: {}, id: {})", *message, channel, user),
                );

                wasm_bindgen_futures::spawn_local(async move {
                    let client = reqwest::Client::new();
                    let resp = client
                        .post("https://sched.sinabro.io/api/v1/gpt")
                        .json(&map)
                        .send()
                        .await
                        .unwrap();

                    assert_eq!(resp.status(), 200);

                    let scheds = resp.json::<SchedResponse>().await.unwrap();

                    send.set(false);
                    message.set("".to_string());
                    state.set(SchedResponse {
                        user: scheds.user.clone(),
                        channel: scheds.channel.clone(),
                        data: scheds.data.to_vec(),
                    });
                });
            }
        })
    };

    Ok(html! {
      <div class="bg-white py-8">
        <div class="mx-auto max-w-7xl px-6 pb-10 mb-5">
            <div class="mx-auto max-w-2xl">
                <h2 class="text-3xl font-bold tracking-tight text-gray-900 text-4xl mt-6">{"Hello, "}{state.user.to_string()}</h2>
                <p class="mt-2 text-lg leading-8 text-gray-600">{"Schedules registered in the "}{state.channel.to_string()}{" channel after today"}</p>
            </div>
            <div class="mx-auto mt-10 grid max-w-2xl grid-cols-1 gap-x-8 gap-y-10 border-t border-gray-200 pt-10">
            {for state.data.iter().map(|sched| {
                html! {<Item user={sched.id.clone()} sched={sched.sched.clone()} date_at={sched.date_at.clone()} />}
            })}
            </div>
        </div>

        <div  class="relative">
            <div class="fixed bottom-0 left-0 right-0 bg-white border-t border-gray-200">
                <div class="mx-auto max-w-7xl px-6 py-3 flex gap-x-4">
                    <label for="command" class="sr-only">{"command"}</label>
                    <input {onchange} value={(*message).clone()} id="command" name="command" type="text" required=true class="min-w-0 flex-auto rounded-md border-0 bg-white/5 px-3.5 py-2 shadow-sm ring-1 ring-inset ring-white/10 focus:ring-2 focus:ring-inset focus:ring-indigo-500" placeholder="When and what schedule would you like to register?" />
                    if *send {
                        <button class="flex-none rounded-md bg-stone-300 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm">{"Proc"}</button>
                    } else {
                        <button {onclick} type="submit" class="flex-none rounded-md bg-stone-900 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-stone-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-500">{"Send"}</button>
                    }
                </div>
            </div>
        </div>
      </div>
    })
}

#[function_component]
pub fn Comp() -> HtmlResult {
    let ctx = use_context::<Auth>().unwrap();
    let Auth {
        user: _,
        channel: _,
        mut token,
    } = ctx;
    let token_clone = token.clone();
    let token_state = use_transitive_state!(|_| -> String { token_clone }, ())?;

    if let Some(token_state) = token_state {
        token = token_state.to_string();
    }

    Ok(html! {
        match token.is_empty() {
            true => html! {<Login />},
            false => html! {<Content />},
        }
    })
}

#[function_component]
pub fn Home() -> Html {
    let fallback = html! {<div>{"Loading..."}</div>};

    html! {
        <Suspense fallback={fallback}>
            <Comp />
        </Suspense>
    }
}
