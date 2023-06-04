mod components;
mod pages;

use std::collections::HashMap;

use yew::prelude::*;
use yew_router::{
    history::{AnyHistory, History, MemoryHistory},
    prelude::*,
};

use crate::{
    components::footer::Footer,
    pages::{home::Home, not_found::PageNotFound},
};

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
    html! {
        <BrowserRouter>
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
    pub token: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Auth {
    pub token: String,
}

#[function_component]
pub fn ServerApp(props: &ServerAppProps) -> Html {
    let ctx = use_state(|| Auth {
        token: props.token.clone(),
    });

    let history = AnyHistory::from(MemoryHistory::new());
    history
        .push_with_query(&*props.url, &props.queries)
        .unwrap();

    html! {
        <Router history={history}>

                <main>
                <ContextProvider<Auth> context={(*ctx).clone()}>
                    <Switch<Route> render={switch} />
                    </ContextProvider<Auth>>
                </main>

                <Footer />

        </Router>
    }
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home  /> },
        Route::NotFound => html! { <PageNotFound /> },
    }
}
