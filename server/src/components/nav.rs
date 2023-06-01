use yew::prelude::*;
use yew_router::prelude::*;

use crate::Route;

#[function_component]
pub fn Nav() -> Html {
    html! {
        <nav>
            <ul>
                <li><Link<Route> to={Route::Home}> {"home"} </Link<Route>></li>
                <li><Link<Route> to={Route::Count}> {"count"} </Link<Route>></li>
            </ul>
        </nav>
    }
}
