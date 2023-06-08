mod components;
mod pages;

use std::collections::HashMap;

use yew::prelude::*;
use yew_router::{
    history::{AnyHistory, History, MemoryHistory},
    prelude::*,
};

use crate::pages::{home::Home, not_found::PageNotFound};

#[derive(Routable, PartialEq, Eq, Clone, Debug)]
pub enum Route {
    #[at("/")]
    Home,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component]
pub fn App() -> Html {
    let ctx = use_state(|| Auth {
        ..Default::default()
    });

    html! {
        <ContextProvider<Auth> context={(*ctx).clone()}>
        <BrowserRouter>
            <main>
                <Switch<Route> render={switch} />
            </main>
        </BrowserRouter>
        </ContextProvider<Auth>>
    }
}

#[derive(Properties, PartialEq, Eq, Debug)]
pub struct ServerAppProps {
    pub url: AttrValue,
    pub queries: HashMap<String, String>,
    pub user: String,
    pub channel: String,
    pub token: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Auth {
    pub user: String,
    pub channel: String,
    pub token: String,
}

#[function_component]
pub fn ServerApp(props: &ServerAppProps) -> Html {
    let ctx = use_state(|| Auth {
        user: props.user.to_owned(),
        channel: props.channel.to_owned(),
        token: props.token.to_owned(),
    });

    let history = AnyHistory::from(MemoryHistory::new());
    history
        .push_with_query(&*props.url, &props.queries)
        .unwrap();

    html! {
        <ContextProvider<Auth> context={(*ctx).clone()}>
            <Router history={history}>
                <main>
                    <Switch<Route> render={switch} />
                </main>
            </Router>
        </ContextProvider<Auth>>
    }
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home  /> },
        Route::NotFound => html! { <PageNotFound /> },
    }
}
