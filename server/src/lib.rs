mod components;
mod pages;

use std::collections::HashMap;

use yew::prelude::*;
use yew_router::{
    history::{AnyHistory, History, MemoryHistory},
    prelude::*,
};

use crate::{
    components::{footer::Footer, nav::Nav},
    pages::{home::Home, not_found::PageNotFound},
};

#[derive(Routable, PartialEq, Eq, Clone, Debug)]
pub enum Route {
    #[at("/")]
    Home,
    #[at("/count")]
    Count,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component]
pub fn Count() -> Html {
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
            <div>
                <p> {"current count: "} {*state} </p>
                <button onclick={incr_counter}> {"+"} </button>
                <button onclick={decr_counter}> {"-"} </button>
            </div>
        </>
    }
}

#[function_component]
pub fn App() -> Html {
    html! {
        <BrowserRouter>
            <Nav />

            <main>
                <Switch<Route> render={switch} />
            </main>

            <Footer />
        </BrowserRouter>
    }
}

#[derive(Properties, PartialEq, Eq, Debug)]
pub struct ServerAppProps {
    pub url: AttrValue,
    pub queries: HashMap<String, String>,
}

#[function_component]
pub fn ServerApp(props: &ServerAppProps) -> Html {
    let history = AnyHistory::from(MemoryHistory::new());
    history
        .push_with_query(&*props.url, &props.queries)
        .unwrap();

    html! {
        <Router history={history}>
            <Nav />

            <main>
                <Switch<Route> render={switch} />
            </main>

            <Footer />
        </Router>
    }
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home /> },
        Route::Count => html! { <Count /> },
        Route::NotFound => html! { <PageNotFound /> },
    }
}
