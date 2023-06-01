use stylist::yew::styled_component;
use yew::prelude::*;

#[styled_component]
pub fn Home() -> Html {
    html! {
        <>
            <div class={css!("color: red;")}>{"Hello World!"}</div>
        </>
    }
}
