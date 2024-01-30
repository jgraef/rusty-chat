use std::str::FromStr;

use leptos::{
    component,
    create_rw_signal,
    view,
    with,
    Callback,
    IntoView,
    MaybeSignal,
    RwSignal,
    Signal,
    SignalSet,
};
use wasm_bindgen::JsCast;
use web_sys::Event;

use crate::{
    state::ConversationParameters,
    utils::non_empty,
};

#[component]
pub fn ConversationParametersInputGroup(
    #[prop(into, optional)] value: MaybeSignal<ConversationParameters>,
    #[prop(into, optional)] on_system_prompt_input: Option<Callback<Option<String>>>,
    #[prop(into, optional)] on_token_limit_input: Option<Callback<Option<usize>>>,
    #[prop(into, optional)] on_temperature_input: Option<Callback<Option<f32>>>,
    #[prop(into, optional)] on_top_k_input: Option<Callback<Option<usize>>>,
    #[prop(into, optional)] on_top_p_input: Option<Callback<Option<f32>>>,
    #[prop(into, optional)] on_repetition_penalty_input: Option<Callback<Option<f32>>>,
    #[prop(into, optional)] on_start_response_with_input: Option<Callback<Option<String>>>,
    #[prop(into, optional)] hide_system_prompt: Signal<bool>,
) -> impl IntoView {
    struct Error(String);

    fn on_input<T: FromStr>(
        callback: Option<Callback<Option<T>>>,
        event: &Event,
        set_invalid: Option<RwSignal<bool>>,
    ) {
        let element = event
            .target()
            .unwrap()
            .unchecked_into::<web_sys::HtmlInputElement>();

        let mut valid = element.check_validity();

        let value = if valid {
            let value = element.value();
            let value = non_empty(value).map(|s| s.parse::<T>()).transpose();
            if value.is_err() {
                valid = false;
            }
            value.ok().flatten()
        }
        else {
            None
        };

        if let Some(callback) = callback {
            callback(value);
        }

        if let Some(set_invalid) = set_invalid {
            set_invalid.set(!valid);
        }
    }

    let invalid_token_limit = create_rw_signal(false);
    let invalid_temperature = create_rw_signal(false);
    let invalid_top_k = create_rw_signal(false);
    let invalid_top_p = create_rw_signal(false);
    let invalid_repetition_penalty = create_rw_signal(false);

    view! {
        <div class="input-group mb-3" class:visually-hidden=hide_system_prompt>
            <span class="input-group-text">"System Prompt"</span>
            <textarea
                class="form-control"
                rows="3"
                on:input=move |event| on_input(on_system_prompt_input, &event, None)
            >
                {with!(|value| value.system_prompt.clone())}
            </textarea>
        </div>
        <div class="input-group mb-3">
            <span class="input-group-text">"Start response with"</span>
            <input
                type="text"
                class="form-control"
                placeholder="Sure thing!"
                value=with!(|value| value.start_response_with.clone())
                on:input=move |event| on_input(on_start_response_with_input, &event, None) />
        </div>
        <div class="d-flex flex-row mb-3">
            <div class="input-group me-3">
                <span class="input-group-text">"Temperature"</span>
                <input
                    type="number"
                    class="form-control"
                    class:is-invalid=invalid_temperature
                    value=with!(|value| value.temperature)
                    on:input=move |event| on_input(on_temperature_input, &event, Some(invalid_temperature))
                />
            </div>
            <div class="input-group me-3">
                <span class="input-group-text">"Top K"</span>
                <input
                    type="number"
                    class="form-control"
                    class:is-invalid=invalid_top_k
                    value=with!(|value| value.top_k)
                    on:input=move |event| on_input(on_top_k_input, &event, Some(invalid_top_k))
                />
            </div>
            <div class="input-group me-3">
                <span class="input-group-text">"Top P"</span>
                <input
                    type="number"
                    class="form-control"
                    class:is-invalid=invalid_top_p
                    value=with!(|value| value.top_p)
                    on:input=move |event| on_input(on_top_p_input, &event, Some(invalid_top_p)) />
            </div>
            <div class="input-group me-3">
                <span class="input-group-text">"Repetition penalty"</span>
                <input
                    type="number"
                    class="form-control"
                    class:is-invalid=invalid_repetition_penalty
                    value=with!(|value| value.repetition_penalty)
                    on:input=move |event| on_input(on_repetition_penalty_input, &event, Some(invalid_repetition_penalty)) />
            </div>
            <div class="input-group">
                <span class="input-group-text">"Token limit"</span>
                <input
                    type="number"
                    class="form-control"
                    class:is-invalid=invalid_token_limit
                    value={with!(|value| value.token_limit)}
                    on:input=move |event| on_input(on_token_limit_input, &event, Some(invalid_token_limit))
                />
            </div>
        </div>
    }
}
