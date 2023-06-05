use yew::prelude::*;

#[function_component]
pub fn PageNotFound() -> Html {
    html! {
        <div class="grid min-h-full place-items-center bg-white px-6 py-24 sm:py-32 lg:px-8">
            <div class="text-center">
                <p class="text-base font-semibold text-gray-600">{"404"}</p>
                <h1 class="mt-4 text-3xl font-bold tracking-tight text-gray-900 sm:text-5xl">{"Page not found"}</h1>
                <p class="mt-6 text-base leading-7 text-gray-600">{"요청된 페이지가 보이질 않아요."}<br/>{"주소를 한번 더 확인해 주세요."}</p>
                <div class="mt-10 items-center justify-center">
                    <div>
                        <a href="https://sched.sinabro.io" class="rounded-md bg-stone-900 px-3.5 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-stone-700 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600">
                            {"홈으로 돌아가기"}
                        </a>
                    </div>
                    <div class="mt-6">
                        <a href="#" class="text-sm font-semibold text-gray-900">{"문의하러 가기"}<span aria-hidden="true">{"→"}</span></a>
                    </div>
                </div>
            </div>
        </div>
    }
}
