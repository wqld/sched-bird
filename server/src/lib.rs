use yew::prelude::*;

#[function_component]
pub fn App() -> Html {
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
        <div>
            <div>
                <p> {"current count: "} {*state} </p>
                <button onclick={incr_counter}> {"+"} </button>
                <button onclick={decr_counter}> {"-"} </button>
            </div>
        </div>
    }
}
