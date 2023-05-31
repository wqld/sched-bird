use tokio::task::{spawn_blocking, LocalSet};
use yew::prelude::*;

use crate::sched::Sched;

#[derive(Clone, Properties, PartialEq)]
pub struct AppProps {
    pub user: String,
    pub scheds: Vec<Sched>,
}

#[function_component]
fn App(props: &AppProps) -> Html {
    let state = use_state(|| 0);

    let incr_counter = {
        let state = state.clone();
        Callback::from(move |_| state.set(*state + 1))
    };

    let decr_counter = {
        let state = state.clone();
        Callback::from(move |_| state.set(*state - 1))
    };

    html! {
        <>
            <h1>{ format!("Hello World! {}", props.user) }</h1>
            <div>
                {
                    for props.scheds.iter().map(|sched| {
                        html! {
                            <div>
                                <p>{ format!("{} {} {}", sched.date_at.to_string(), sched.id, sched.sched) }</p>
                            </div>
                        }
                    })
                }
            </div>
            <div>
                <p> {"current count: "} {*state} </p>
                <button onclick={incr_counter}> {"+"} </button>
                <button onclick={decr_counter}> {"-"} </button>
            </div>
        </>
    }
}

pub async fn render_app(user: String, scheds: Vec<Sched>) -> String {
    spawn_blocking(move || {
        use tokio::runtime::Builder;
        let set = LocalSet::new();

        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        set.block_on(&rt, async {
            let renderer = yew::ServerRenderer::<App>::with_props(|| AppProps { user, scheds });
            renderer.render().await
        })
    })
    .await
    .expect("error rendering app")
}
