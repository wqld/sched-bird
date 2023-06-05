use serde::{Deserialize, Serialize};
use yew::prelude::*;

use crate::{
    components::{item::Item, login::Login},
    Auth,
};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Sched {
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
    println!("ctx: {:?}", ctx);

    let user = ctx.user;
    let channel: &'static str = Box::leak(ctx.channel.into_boxed_str());

    let scheds = use_prepared_state!(
        async move |_| -> SchedResponse { fetch_sched(&ctx.token, channel).await },
        ()
    )?
    .unwrap();

    Ok(html! {
      <div class="bg-white py-8">
        <div class="mx-auto max-w-7xl px-6">
            <div class="mx-auto max-w-2xl">
                <h2 class="text-3xl font-bold tracking-tight text-gray-900 text-4xl mt-8">{"Hello, "}{user}</h2>
                <p class="mt-2 text-lg leading-8 text-gray-600">{channel}{" 채널에 등록된 오늘부터의 일정이에요."}</p>
            </div>
            <div class="mx-auto mt-10 grid max-w-2xl grid-cols-1 gap-x-8 gap-y-16 border-t border-gray-200 pt-10">
            {for scheds.data.iter().map(|sched| {
                html! {<Item user={sched.id.clone()} sched={sched.sched.clone()} date_at={sched.date_at.clone()} />}
            })}
            </div>
        </div>

        <div  class="relative">
            <div class="fixed bottom-0 left-0 right-0">
                <div class="mx-8 my-8 flex max-w-full gap-x-4">
                    <label for="command" class="sr-only">{"command"}</label>
                    <input id="command" name="command" type="text" required=true class="min-w-0 flex-auto rounded-md border-0 bg-white/5 px-3.5 py-2 text-white shadow-sm ring-1 ring-inset ring-white/10 focus:ring-2 focus:ring-inset focus:ring-indigo-500 sm:text-sm sm:leading-6" placeholder="언제, 어떤 일정을 등록할까요?" />
                    <button type="submit" class="flex-none rounded-md bg-stone-900 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-stone-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-500">{"보내기"}</button>
                </div>
            </div>
        </div>
      </div>
    })
}

#[function_component]
pub fn Home() -> Html {
    let ctx = use_context::<Auth>().unwrap();
    let fallback = html! {<div>{"Loading..."}</div>};

    html! {
        <Suspense fallback={fallback}>
            { match ctx.token.is_empty() {
                true => html! {<Login />},
                false => html! {<Content />},
            }}
        </Suspense>
    }
}
