use chrono::Local;
use leptos::{
    component,
    create_node_ref,
    ev::SubmitEvent,
    event_target_value,
    expect_context,
    html::Input,
    view,
    with,
    CollectView,
    For,
    IntoView,
    Signal,
    SignalGet,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWith,
    SignalWithUntracked,
};
use leptos_router::{
    use_navigate,
    A,
};

use super::{
    conversation::ConversationParametersInputGroup,
    push_user_message,
    request_conversation_title,
    BootstrapIcon,
    Context,
};
use crate::{
    config::{
        BUILD_CONFIG,
        GITHUB_ISSUES_PAGE,
    },
    state::{
        use_conversation,
        Conversation,
        ConversationId,
        StorageSignals,
    },
};

#[component]
pub fn Home() -> impl IntoView {
    let Context {
        is_loading,
        settings,
        home,
        update_home,
        update_conversations,
        ..
    } = expect_context();

    let user_message_input = create_node_ref::<Input>();

    let current_model = Signal::derive(move || with!(|home| home.selected_model.clone()));

    let current_model_name = Signal::derive(move || {
        with!(|current_model, settings| {
            settings
                .models
                .get(current_model)
                .unwrap()
                .display_name()
                .to_owned()
        })
    });

    let hide_system_prompt_input = Signal::derive(move || {
        with!(|settings, current_model| {
            settings
                .models
                .get(current_model)
                .map(|model| !model.chat_template.supports_system_prompt())
                .unwrap_or_default()
        })
    });

    let start_chat = move |user_message: String, conversation_parameters| {
        let now = Local::now();

        let current_model = current_model.get_untracked();

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

        let Some(user_message_input) = user_message_input.get_untracked()
        else {
            log::error!("user_message_input missing");
            return;
        };

        let user_message = user_message_input.value();
        if user_message.is_empty() {
            return;
        }

        let Some(conversation_parameters) = update_home
            .try_update(|home| {
                home.user_message = "".to_owned();
                let conversation_parameters = home.conversation_parameters.clone();
                Some(conversation_parameters)
            })
            .flatten()
        else {
            log::error!("home write signal went dead");
            return;
        };

        start_chat(user_message, conversation_parameters);
    };

    let disable_send = Signal::derive(move || is_loading.get());

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
                            "Found a bug? Please let us know on "
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
                            BUILD_CONFIG.examples.iter().map(|example| {
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
                <div class="mb-3 dropup flex-grow-1">
                    <div class="input-group" data-bs-toggle="dropdown">
                        <span class="input-group-text">"Model"</span>
                        <input
                            class="form-control"
                            value=current_model_name
                            readonly
                        />
                    </div>
                    <div class="dropdown-menu w-100">
                        /*<For
                            each=move || with!(|model_options| model_options.iter().cloned().collect::<Vec<_>>())
                            key=move |(id, _)| id.clone()
                            children=move |(id, display_name)| {
                                view! {
                                    <li>
                                        <button
                                            type="button"
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
                        />*/
                        <div class="overflow-y-scroll" style="max-height: 50vh;">
                            <For
                                each=move || with!(|settings| {
                                    let mut items = settings.models
                                        .iter()
                                        .map(|(id, model)| (id.clone(), model.display_name().to_lowercase()))
                                        .collect::<Vec<_>>();
                                    items.sort_by_cached_key(|(_, name)| name.clone());
                                    items
                                })
                                key=|(model_id, _)| model_id.clone()
                                children=move |(model_id, _)| {
                                    let model_name = settings.with(|settings| settings.models.get(&model_id).unwrap().display_name().to_owned());
                                    view!{
                                        <button
                                            type="button"
                                            class="dropdown-item"
                                            class:active=move || with!(|current_model| current_model == &model_id)
                                            on:click={
                                                let model_id = model_id.clone();
                                                move |_| {
                                                    let model_id = model_id.clone();
                                                    update_home.update(move |home| home.selected_model = model_id);
                                                }
                                            }
                                        >
                                            {model_name}
                                        </button>
                                    }
                                }
                            />
                        </div>
                        <small class="dropdown-header my-0 mx-3 p-0">
                            "Add more models under "
                            <A href="/settings/models">
                                "Settings"
                            </A>
                        </small>
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
