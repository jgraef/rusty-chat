use chrono::Local;
use lazy_static::lazy_static;
use leptos::{
    component,
    create_node_ref,
    create_rw_signal,
    ev::SubmitEvent,
    event_target_value,
    expect_context,
    html::Input,
    view,
    with,
    CollectView,
    For,
    IntoView,
    RwSignal,
    Signal,
    SignalGet,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWithUntracked,
};
use leptos_router::{
    use_navigate,
    A,
};
use leptos_use::{
    use_debounce_fn_with_arg_and_options,
    DebounceOptions,
};
use serde::Deserialize;
use web_sys::Event;

use super::{
    conversation::ConversationParametersInputGroup,
    push_user_message,
    request_conversation_title,
    BootstrapIcon,
    Context,
};
use crate::{
    state::{
        use_conversation,
        use_conversations,
        use_home,
        use_settings,
        Conversation,
        ConversationId,
        ModelId,
        StorageSignals,
    },
    utils::non_empty,
    GITHUB_ISSUES_PAGE,
};

lazy_static! {
    static ref EXAMPLES: Vec<String> = {
        #[derive(Debug, Deserialize)]
        struct Examples {
            examples: Vec<String>,
        }

        let examples: Examples =
            toml::from_str(include_str!("../../examples.toml")).expect("invalid examples.toml");
        examples.examples
    };
}

#[component]
pub fn Home() -> impl IntoView {
    let Context { is_loading, .. } = expect_context();

    let user_message_input = create_node_ref::<Input>();

    let StorageSignals { read: settings, .. } = use_settings();

    let StorageSignals {
        read: home,
        write: update_home,
        ..
    } = use_home();
    let current_model = Signal::derive(move || with!(|home| home.selected_model.clone()));

    let hide_system_prompt_input = Signal::derive(move || {
        with!(|settings, current_model| {
            current_model
                .as_ref()
                .and_then(|model_id| {
                    settings
                        .models
                        .get(model_id)
                        .map(|model| !model.chat_template.supports_system_prompt())
                })
                .unwrap_or_default()
        })
    });

    let StorageSignals {
        write: update_conversations,
        ..
    } = use_conversations();

    let start_chat = move |user_message: String, conversation_parameters| {
        let now = Local::now();

        // currently we don't support starting chats without a model. but we might
        // later, once we have backends that have a fixed model
        let Some(current_model) = current_model.get_untracked()
        else {
            return;
        };

        let conversation_id = ConversationId::new();
        let conversation = Conversation {
            id: conversation_id,
            model_id: Some(current_model),
            title: None,
            timestamp_started: now,
            timestamp_last_interaction: now,
            messages: vec![],
            conversation_parameters,
            user_message: "".to_owned(),
        };

        update_conversations.update(|conversations| {
            conversations.insert(conversation_id);
        });

        let StorageSignals {
            write: update_conversation,
            ..
        } = use_conversation(conversation_id);
        update_conversation.set(Some(conversation));

        request_conversation_title(conversation_id, &user_message);
        push_user_message(conversation_id, user_message);

        use_navigate()(
            &format!("/conversation/{conversation_id}"),
            Default::default(),
        );
    };

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();

        let Some((user_message, conversation_parameters)) = update_home
            .try_update(|home| {
                let user_message = non_empty(std::mem::replace(
                    &mut home.user_message,
                    Default::default(),
                ))?;
                let conversation_parameters = home.conversation_parameters.clone();
                Some((user_message, conversation_parameters))
            })
            .flatten()
        else {
            return;
        };

        start_chat(user_message, conversation_parameters);
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ModelInputStatus {
        NoModel,
        Checking,
        Loadable,
        NotLoadable,
        NotFound,
    }

    impl ModelInputStatus {
        fn is_valid(&self) -> bool {
            *self == Self::Loadable
        }

        fn is_invalid(&self) -> bool {
            *self == Self::NotFound || *self == Self::NotLoadable
        }

        fn is_loading(&self) -> bool {
            *self == Self::Checking
        }
    }

    let model_options: RwSignal<Vec<(ModelId, String)>> = create_rw_signal(vec![]);
    // todo: check if stored model_id is empty or valid/invalid to initialize these.
    let model_input_is_invalid = create_rw_signal(false);
    let model_input_is_empty = create_rw_signal(current_model.with_untracked(|id| id.is_none()));

    let set_current_model = move |model_id: Option<ModelId>| {
        settings.with_untracked(move |settings| {
            let mut models = if let Some(model_id) = model_id {
                let mut models = vec![];
                let mut exact_match = false;
                let model_id_lowercase = model_id.0.to_lowercase();

                for (id, model) in settings.models.range(model_id.clone()..) {
                    if id == &model_id {
                        exact_match = true;
                    }

                    if id.0.to_lowercase().contains(&model_id_lowercase)
                        || model
                            .name
                            .as_ref()
                            .map(|name| name.to_lowercase().contains(&model_id_lowercase))
                            .unwrap_or_default()
                    {
                        models.push((id.clone(), model.display_name().to_owned()));
                    }
                }

                model_input_is_invalid.set(!exact_match);
                model_input_is_empty.set(false);

                if exact_match {
                    update_home.update(move |home| {
                        home.selected_model = Some(model_id);
                    });
                }

                models
            }
            else {
                model_input_is_empty.set(true);
                model_input_is_invalid.set(false);

                update_home.update(move |home| {
                    home.selected_model = None;
                });

                settings
                    .models
                    .iter()
                    .map(|(id, model)| (id.clone(), model.display_name().to_owned()))
                    .collect()
            };

            models.sort_by_cached_key(|(_, display_name)| display_name.to_lowercase());

            model_options.set(models);
        });
    };

    set_current_model(current_model.get_untracked());

    let on_model_input = move |event: Event| {
        let model_id = non_empty(event_target_value(&event)).map(ModelId);
        log::debug!("model input: '{model_id:?}'");

        set_current_model(model_id);
    };

    let on_model_input_debounced =
        use_debounce_fn_with_arg_and_options(on_model_input, 100.0, DebounceOptions::default());

    let disable_send = Signal::derive(move || {
        is_loading.get() || model_input_is_empty.get() || model_input_is_invalid.get()
    });

    view! {
        <div class="d-flex flex-column h-100 w-100">
            <div class="d-flex flex-column flex-grow-1 overflow-scroll">
                <div class="d-flex flex-column w-50 m-auto welcome">
                    <div class="d-flex flex-column mb-4">
                        <h4>"Welcome!"</h4>
                        <p class="mt-2 mx-4">
                            "Welcome to RustyChat! RustyChat is a client-only web app to talk to any model that is available through the free"
                            <a href="https://huggingface.co/" target="_blank" class="text-decoration-none ps-1">"🤗"</a>
                            <a href="https://huggingface.co/" target="_blank" class="pe-1">
                                "Hugging Face"
                                <BootstrapIcon icon="link-45deg" />
                            </a>
                            "API. And all your data is kept stored here in your browser."
                        </p>
                        <p class="mt-2 mx-4">
                            "The app starts out with a few models. We recommend "
                            <i>
                                "Nous Hermes 2"
                            </i>
                            ". You can add more models under "
                            <A href="/settings/models">
                                "Settings"
                            </A>
                            "."
                        </p>
                        <p class="mt-2 mx-4">
                            "You found a bug? Please let us know on "
                            <a href=GITHUB_ISSUES_PAGE target="_blank">
                                "GitHub"
                                <BootstrapIcon icon="link-45deg" />
                            </a>
                            "."
                        </p>
                    </div>
                    <div class="d-flex flex-column">
                        <h4>"Examples"</h4>
                        {
                            EXAMPLES.iter().map(|example| {
                                view!{
                                    <button
                                        type="button"
                                        class="btn btn-outline-secondary p-2 mt-2 mx-4"
                                        on:click=move |_| {
                                            log::debug!("example: {example}");
                                            start_chat(example.to_owned(), Default::default());
                                        }
                                    >
                                        {example}
                                    </button>
                                }
                            }).collect_view()
                        }
                    </div>
                </div>
            </div>

            <div class="d-flex flex-column px-3 pt-3 shadow-lg">
                <div class="collapse pb-2" id="startChatAdvancedContainer">
                    <ConversationParametersInputGroup
                        value=home.with_untracked(|home| home.conversation_parameters.clone())
                        on_system_prompt_input=move |value| update_home.update(move |home| home.conversation_parameters.system_prompt = value)
                        on_start_response_with_input=move |value| update_home.update(move |home| home.conversation_parameters.start_response_with = value)
                        on_temperature_input=move |value| update_home.update(move |home| home.conversation_parameters.temperature = value)
                        on_top_k_input=move |value| update_home.update(move |home| home.conversation_parameters.top_k = value)
                        on_top_p_input=move |value| update_home.update(move |home| home.conversation_parameters.top_p = value)
                        on_repetition_penalty_input=move |value| update_home.update(move |home| home.conversation_parameters.repetition_penalty = value)
                        on_token_limit_input=move |value| update_home.update(move |home| home.conversation_parameters.token_limit = value)
                        hide_system_prompt=hide_system_prompt_input
                    />
                </div>
                <div class="d-flex flex-row mb-3">
                    /*<div class="input-group me-3 flex-shrink w-25">
                        <select class="form-select" aria-label="Select a backend">
                            <option selected>"Hugging Face"</option>
                            <option value="llama-cpp">"llama.cpp"</option>
                            <option value="llama-cpp-rs">"llama.cpp-rs"</option>
                        </select>
                    </div>*/
                    <div class="input-group flex-grow-1 dropup">
                        <span class="input-group-text">"Model"</span>
                        /*<select class="form-select" aria-label="Select a model to chat with" on:change=on_model_selected>
                            <For
                                each=move || with!(|settings| settings.models.keys().cloned().collect::<Vec<_>>())
                                key=|model_id| model_id.clone()
                                children=move |model_id| {
                                    let model_id_str = model_id.to_string();
                                    view!{
                                        <option selected=move || current_model.with(|current| current == &model_id) value={model_id_str.clone()}>{model_id_str}</option>
                                    }
                                }
                            />
                        </select>*/
                        <input
                            class="form-control"
                            class:is-invalid=model_input_is_invalid
                            placeholder="Select model..."
                            prop:value=move || current_model.get().map(|id| id.0).unwrap_or_default()
                            data-bs-toggle="dropdown"
                            on:input=move |event| { on_model_input_debounced(event); }
                        />
                        <ul
                            class="dropdown-menu w-100"
                            class:visually-hidden=move || with!(|model_options| model_options.is_empty())
                        >
                            <For
                                each=move || with!(|model_options| model_options.iter().cloned().collect::<Vec<_>>())
                                key=move |(id, _)| id.clone()
                                children=move |(id, display_name)| {
                                    view! {
                                        <li>
                                            <button
                                                type="button"
                                                href="#"
                                                class="dropdown-item"
                                                on:click=move |_| {
                                                    set_current_model(Some(id.clone()));
                                                }
                                            >
                                                {display_name}
                                            </button>
                                        </li>
                                    }
                                }
                            />
                        </ul>
                    </div>
                </div>
                <form on:submit=on_submit>
                    <div class="input-group input-group-lg mb-3">
                        <input
                            type="text"
                            class="form-control"
                            placeholder="Ask anything"
                            value=home.with_untracked(|home| home.user_message.clone())
                            node_ref=user_message_input
                            on:input=move |event| {
                                let user_message = event_target_value(&event);
                                update_home.update(|home| home.user_message = user_message);
                            }
                        />
                        <button
                            class="btn btn-outline-secondary"
                            type="submit"
                            disabled=disable_send
                        >
                            <BootstrapIcon icon="send" />
                        </button>
                        <button class="btn btn-outline-secondary" type="button" data-bs-toggle="collapse" data-bs-target="#startChatAdvancedContainer"><BootstrapIcon icon="three-dots" /></button>
                    </div>
                </form>
            </div>
        </div>
    }
}
