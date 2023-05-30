use tokio::task::{spawn_blocking, LocalSet};
use yew::prelude::*;

#[derive(Clone, Properties, PartialEq)]
pub struct AppProps {
    pub user: String,
}

#[function_component]
fn App(props: &AppProps) -> Html {
    html! {
        <div>
            <h1>{ format!("Hello World! {}", props.user) }</h1>
        </div>
    }
}

pub async fn render_app(id: String) -> String {
    let content = spawn_blocking(move || {
        use tokio::runtime::Builder;
        let set = LocalSet::new();

        let rt = Builder::new_current_thread().enable_all().build().unwrap();

        set.block_on(&rt, async {
            let renderer = yew::ServerRenderer::<App>::with_props(|| AppProps { user: id });
            renderer.render().await
        })
    })
    .await
    .expect("error rendering app");

    content
}
