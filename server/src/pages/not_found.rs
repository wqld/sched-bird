use yew::prelude::*;

#[function_component]
pub fn PageNotFound() -> Html {
    html! {
        <section>
            <div>
                <h1>{ "Page not found" }</h1>
                <h2>{ "Page page does not seem to exist" }</h2>
            </div>
        </section>
    }
}
