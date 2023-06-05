use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct ItemProps {
    pub user: String,
    pub date_at: String,
    pub sched: String,
}

#[function_component]
pub fn Item(props: &ItemProps) -> Html {
    html! {
        <article class="flex max-w-xl flex-col items-start justify-between">
            <div class="flex items-center gap-x-4 text-xs">
              <p class="text-gray-500">{props.date_at.to_owned()}</p>
              <a href="#" class="relative z-10 rounded-full bg-gray-50 px-3 py-1.5 font-medium text-gray-600 hover:bg-gray-100">{props.user.to_owned()}</a>
            </div>
            <div class="group relative">
              <h3 class="mt-3 text-lg font-semibold leading-6 text-gray-900 group-hover:text-gray-600">
                <a href="#">
                  <span class="absolute inset-0"></span>
                    {props.sched.to_owned()}
                </a>
              </h3>
            </div>
        </article>
    }
}
