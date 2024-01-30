use std::{
    fmt::Display,
    str::FromStr,
    sync::Arc,
};

use chrono::Local;
use futures::{
    stream::TryStreamExt,
    FutureExt,
};
use leptos::{
    component,
    create_effect,
    create_memo,
    create_node_ref,
    create_rw_signal,
    ev::SubmitEvent,
    event_target_value,
    html::{
        Div,
        Input,
        Textarea,
    },
    spawn_local,
    update,
    view,
    with,
    Callable,
    Callback,
    Children,
    DynAttrs,
    For,
    IntoView,
    MaybeSignal,
    NodeRef,
    Oco,
    RwSignal,
    Signal,
    SignalGet,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWith,
    SignalWithUntracked,
    WriteSignal,
};
use leptos_meta::{
    provide_meta_context,
    Html,
};
use leptos_router::{
    use_navigate,
    use_params_map,
    Route,
    Router,
    Routes,
    ToHref,
    A,
};
use leptos_use::{
    use_color_mode,
    ColorMode,
    UseColorModeReturn,
};
use uuid::Uuid;
use wasm_bindgen::JsCast;
use web_sys::{
    Event,
    ScrollLogicalPosition,
};

use crate::{
    state::{
        clear_storage,
        delete_storage,
        use_conversation,
        use_conversations,
        use_home,
        use_message,
        use_settings,
        use_version,
        AppVersion,
        Conversation,
        ConversationId,
        ConversationParameters,
        Message,
        MessageId,
        ModelId,
        Role,
        StorageKey,
        StorageSignals,
    },
    utils::{
        get_input_value,
        non_empty,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("text generation error")]
    TextGeneration(#[from] hf_textgen::Error),
}

#[derive(Clone)]
struct Context {
    pub api: Arc<hf_textgen::Api>,
    pub is_loading: RwSignal<bool>,
}

fn provide_context() {
    let version = use_version();
    let stored_version = version.read.get_untracked();
    let current_version = AppVersion::default();
    log::info!("stored version: {}", stored_version);
    log::info!("current version: {}", current_version);
    //version.write.set(current_version);

    leptos::provide_context(Context {
        api: Arc::new(hf_textgen::Api::default()),
        is_loading: create_rw_signal(false),
    });
}

fn expect_context() -> Context {
    leptos::expect_context::<Context>()
}

fn push_user_message(conversation_id: ConversationId, user_message: String) {
    let Context {
        api, is_loading, ..
    } = expect_context();

    let message_id = MessageId::new();
    let now = Local::now();

    // create and store message
    let StorageSignals {
        write: set_message, ..
    } = use_message(message_id);
    set_message.set(Some(Message {
        id: message_id,
        role: Role::User,
        text: user_message,
        timestamp: now,
    }));

    // add message to conversation and get model_id and prompt
    let (model_id, prompt, start_response_with) = {
        let StorageSignals {
            write: update_conversation,
            ..
        } = use_conversation(conversation_id);
        let StorageSignals { read: settings, .. } = use_settings();

        update_conversation
            .try_update(move |conversation| {
                let model_id = conversation
                    .model_id
                    .clone()
                    .expect("conversation has no model_id set");

                conversation.messages.push(message_id);
                conversation.timestamp_last_interaction = now;

                let messages = conversation
                    .messages
                    .iter()
                    .filter_map(|message_id| {
                        let StorageSignals { read: message, .. } = use_message(*message_id);
                        message.get_untracked()
                    })
                    .collect::<Vec<_>>();

                let chat_template = settings.with_untracked(|settings| {
                    settings.models.get(&model_id).unwrap().chat_template
                });

                let prompt = chat_template.generate_prompt(
                    conversation
                        .conversation_parameters
                        .system_prompt
                        .as_ref()
                        .map(|s| s.as_str()),
                    &messages,
                    conversation
                        .conversation_parameters
                        .start_response_with
                        .as_ref()
                        .map(|s| s.as_str()),
                );

                (
                    model_id,
                    prompt,
                    conversation
                        .conversation_parameters
                        .start_response_with
                        .clone(),
                )
            })
            .unwrap()
    };

    let mut model = api.text_generation(&model_id.0);
    model.max_new_tokens = Some(2048);

    spawn_local(
        async move {
            is_loading.set(true);

            let mut stream = model.generate(&prompt).await?;

            let message_id = MessageId::new();
            let now = Local::now();

            let StorageSignals {
                write: set_message, ..
            } = use_message(message_id);
            set_message.set(Some(Message {
                id: message_id,
                role: Role::Assitant,
                text: start_response_with.unwrap_or_default(),
                timestamp: now,
            }));

            let StorageSignals {
                write: set_conversation,
                ..
            } = use_conversation(conversation_id);
            set_conversation.update(|conversation| {
                conversation.messages.push(message_id);
                conversation.timestamp_last_interaction = now;
            });

            while let Some(token) = stream.try_next().await? {
                if token.special {
                    continue;
                }

                set_message.update(move |message| {
                    let message = message.as_mut().unwrap();

                    /*let mut text: &str = &token.text;
                    if message.text.is_empty() {
                        text = text.trim_start();
                    }*/

                    message.text.push_str(&token.text);
                });
            }
            Ok(())
        }
        .map(move |result: Result<(), Error>| {
            if let Err(e) = result {
                log::error!("{e}");
            }
            log::debug!("response stream finished");
            is_loading.set(false);
        }),
    );
}

fn request_conversation_title(conversation_id: ConversationId, user_message: &str) {
    let Context { api, .. } = expect_context();

    let mut model = api.text_generation("NousResearch/Nous-Hermes-2-Mixtral-8x7B-DPO");
    model.max_new_tokens = Some(20);

    let prompt = format!(
        r#"<|im_start|>system
Your job is to generate a short descriptive title of a chat conversation between an user and an AI assistant, given the first message from the user.
Start the title with a fitting emoji. Please respond only with the title and nothing else.
<|im_end|>
<|im_start|>user
Message: Write a short poem about AI.
<|im_end|>
<|im_start|>assistant
✨ A Modern Muse
<|im_end|>
<|im_start|>user
Message: {user_message}
<|im_end|>
<|im_start|>assistant
"#
    );

    let StorageSignals {
        write: update_conversation,
        ..
    } = use_conversation(conversation_id);

    spawn_local(
        async move {
            let stream = model.generate(&prompt).await?.text();
            let title = stream.try_collect::<String>().await?;
            let mut lines = title.lines();
            let title = lines.next().unwrap().to_owned();

            log::debug!("generated title: '{title}'");

            update_conversation.update(move |conversation| {
                conversation.title = Some(title);
            });

            Ok(())
        }
        .map(move |result: Result<(), Error>| {
            if let Err(e) = result {
                log::error!("{e}");
            }
        }),
    )
}

#[component]
pub fn BootstrapIcon(#[prop(into)] icon: Oco<'static, str>) -> impl IntoView {
    view! { <i class={format!("bi bi-{icon}")}></i> }
}

#[component]
pub fn NavLink<H: ToHref + 'static>(href: H, children: Children) -> impl IntoView {
    view! {
        <li class="nav-item">
            <A href={href} active_class="active" class="nav-link text-light">
                {children()}
            </A>
        </li>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    provide_context();

    let StorageSignals {
        read: conversations,
        ..
    } = use_conversations();

    let conversations = create_memo(move |_| {
        with!(|conversations| {
            let mut conversations = conversations
                .iter()
                .map(|&id| {
                    let StorageSignals {
                        read: conversation, ..
                    } = use_conversation(id);
                    with!(|conversation| {
                        (
                            id,
                            conversation.title.clone(),
                            conversation.timestamp_last_interaction,
                        )
                    })
                })
                .collect::<Vec<_>>();
            conversations.sort_by_cached_key(|(_, _, ts)| *ts);
            conversations.reverse();
            conversations
        })
    });

    let (bs_theme, toggle_theme, theme_icon) = {
        let UseColorModeReturn { mode, set_mode, .. } = use_color_mode();
        let bs_theme = Signal::derive(move || {
            match mode.get() {
                ColorMode::Dark => "dark",
                _ => "light",
            }
        });
        let toggle_theme = move || {
            let current = mode.get();
            let new = match current {
                ColorMode::Dark => ColorMode::Light,
                _ => ColorMode::Dark,
            };
            set_mode.set(new);
        };
        let theme_icon = Signal::derive(move || {
            match mode.get() {
                ColorMode::Dark => "moon-fill",
                _ => "sun-fill",
            }
        });
        (bs_theme, toggle_theme, theme_icon)
    };

    view! {
        <Html
            attr:data-bs-theme=bs_theme
        />
        <Router>
            <div class="d-flex flex-row" style="height: 100vh; width: 100%">
                <nav class="d-flex flex-column flex-shrink-0 p-3 text-white shadow-lg sidebar">
                    <div class="d-flex flex-row">
                        <A class="d-flex align-items-center mb-3 mb-md-0 me-md-auto text-white text-decoration-none" href="/">
                            <span class="fs-4">"🦀 RustyChat"</span>
                        </A>
                        <small class="d-flex flex-row">
                            <button type="button" class="btn py-0 px-1 m-auto" style="color: white;" on:click=move |_| toggle_theme()>
                                {move || {
                                    let theme_icon = theme_icon.get();
                                    view!{<BootstrapIcon icon=theme_icon />}
                                }}
                            </button>
                            <a href="https://github.com/jgraef/rusty-chat" target="_blank" class="py-0 px-1 m-auto" style="color: white;">
                                <BootstrapIcon icon="github" />
                            </a>
                        </small>
                    </div>
                    <hr />
                    <ul class="nav nav-pills flex-column mb-auto">
                        <For
                            each=conversations
                            key=|(id, _, _)| *id
                            children=move |(id, title, _)| {
                                view! {
                                    <NavLink href=format!("/conversation/{id}")>
                                        <div class="text-nowrap text-truncate" style="width: 200px">
                                            {
                                                if let Some(title) = title {
                                                    view!{{title}}.into_view()
                                                }
                                                else {
                                                    view!{
                                                        <span class="me-2"><BootstrapIcon icon="question-lg" /></span>
                                                        "Untitled"
                                                    }.into_view()
                                                }
                                            }
                                        </div>
                                    </NavLink>
                                }
                            }
                        />
                    </ul>
                    <hr />
                    <ul class="nav nav-pills flex-column">
                        <NavLink href="/settings">
                            <span class="me-2"><BootstrapIcon icon="gear" /></span>
                            "Settings"
                        </NavLink>
                    </ul>
                </nav>
                <main class="w-100 p-0 main">
                    <div class="d-flex flex-column flex-grow-1 h-100" style="max-height: 100vh">
                        <Routes>
                            <Route path="/" view=Home />
                            <Route path="/conversation/:id" view=move || {
                                let params = use_params_map();
                                let id = Signal::derive(move || {
                                    let id = params.with(|p| p.get("id").cloned().unwrap());
                                    let id = Uuid::parse_str(&id).expect("invalid conversation id");
                                    let id = ConversationId::from(id);
                                    id
                                });
                                view!{ <Conversation id=id /> }
                            } />
                            <Route path="/settings" view=Settings />
                            <Route path="" view=NotFound />
                        </Routes>
                    </div>
                </main>
            </div>
        </Router>
    }
}

#[component]
fn Home() -> impl IntoView {
    let user_message_input = create_node_ref::<Input>();

    let StorageSignals { read: settings, .. } = use_settings();

    let StorageSignals {
        read: home,
        write: update_home,
        ..
    } = use_home();
    let current_model = Signal::derive(move || {
        with!(|home| {
            home.selected_model.clone().unwrap_or_else(move || {
                settings
                    .with_untracked(|settings| settings.models.first_key_value().unwrap().0.clone())
            })
        })
    });

    let hide_system_prompt_input = Signal::derive(move || {
        with!(|settings, current_model| {
            !settings
                .models
                .get(current_model)
                .unwrap()
                .chat_template
                .supports_system_prompt()
        })
    });

    let StorageSignals {
        write: update_conversations,
        ..
    } = use_conversations();

    let on_model_selected = move |event| {
        let model_id = ModelId(event_target_value(&event));
        log::debug!("model selected: {model_id}");

        update_home.update(move |home| {
            home.selected_model = Some(model_id);
        });
    };

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();

        let Some((user_message, conversation_parameters)) = update_home.try_update(|home| {
            (
                std::mem::replace(&mut home.user_message, Default::default()),
                home.conversation_parameters.clone(),
            )
        })
        else {
            return;
        };

        let now = Local::now();

        let conversation_id = ConversationId::new();
        let conversation = Conversation {
            id: conversation_id,
            model_id: Some(current_model.get_untracked()),
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
        update_conversation.set(conversation);

        request_conversation_title(conversation_id, &user_message);
        push_user_message(conversation_id, user_message);

        use_navigate()(
            &format!("/conversation/{conversation_id}"),
            Default::default(),
        );
    };

    view! {
        <div class="d-flex flex-column h-100 w-100">
            <div class="mb-auto">
                // TODO: say hello to the user
            </div>
            <form on:submit=on_submit class="p-4 shadow-lg needs-validation" novalidate>
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
                <div class="input-group mb-3">
                    <span class="input-group-text">"Model"</span>
                    <select class="form-select" aria-label="Select a model to chat with" on:change=on_model_selected>
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
                    </select>
                </div>
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
                    <button class="btn btn-outline-secondary" type="submit"><BootstrapIcon icon="send" /></button>
                    <button class="btn btn-outline-secondary" type="button" data-bs-toggle="collapse" data-bs-target="#startChatAdvancedContainer"><BootstrapIcon icon="three-dots" /></button>
                </div>
            </form>
        </div>
    }
}

#[component]
fn Conversation(#[prop(into)] id: MaybeSignal<ConversationId>) -> impl IntoView {
    let Context { is_loading, .. } = expect_context();

    let confirm_delete = create_rw_signal(false);

    let delete_button_clicked = move |_| {
        if confirm_delete.get() {
            log::debug!("delete chat: {}", id.get_untracked());

            use_navigate()("/", Default::default());

            let id = id.get_untracked();

            let StorageSignals {
                write: conversations,
                ..
            } = use_conversations();
            update!(|conversations| {
                conversations.remove(&id);
            });

            let conversation = use_conversation(id);
            let message_ids = conversation
                .read
                .with(|conversation| conversation.messages.clone());
            conversation.delete();

            for message_id in message_ids {
                delete_storage(StorageKey::Message(message_id));
            }
        }
        else {
            confirm_delete.set(true);
        }
    };

    // auto-scrolling
    // this is done by having an empty div at the bottom of the page (right before
    // the spacer) that we scroll into view whenever a change to the conversation
    // happens.

    pub fn scroll_to(target: NodeRef<Div>, smooth: bool) {
        let Some(scroll_target) = target.get_untracked()
        else {
            return;
        };

        let mut scroll_options = web_sys::ScrollIntoViewOptions::new();
        scroll_options.block(ScrollLogicalPosition::End);
        scroll_options.behavior(if smooth {
            web_sys::ScrollBehavior::Smooth
        }
        else {
            web_sys::ScrollBehavior::Instant
        });

        scroll_target.scroll_into_view_with_scroll_into_view_options(&scroll_options);
    }

    view! {
        {move || {
            let StorageSignals { read: conversation, write: update_conversation, .. } = use_conversation(id.get());
            let StorageSignals { read: settings, .. } = use_settings();

            // create effect to auto-scroll
            let scroll_target = create_node_ref::<Div>();
            //let is_initial_scroll = create_rw_signal(true);

            /*create_effect(move |_| {
                conversation.with(|_| ());
                let initial = is_initial_scroll.get_untracked();
                scroll_to(scroll_target, !initial);
                is_initial_scroll.set(false);
            });*/

            // send message

            let on_submit = move |event: SubmitEvent| {
                event.prevent_default();

                let Some((user_message, id)) = update_conversation.try_update(|conversation| (
                    std::mem::replace(&mut conversation.user_message, Default::default()),
                    conversation.id,
                )) else { return; };

                push_user_message(id, user_message);
            };

            let title = Signal::derive(move || {
                with!(|conversation| conversation.title.clone())
            });
            let model_id = Signal::derive(move || {
                with!(|conversation| conversation.model_id.clone().expect("no model"))
            });
            let hide_system_prompt_input = Signal::derive(move || {
                with!(|settings, model_id| {
                    !settings
                        .models
                        .get(model_id)
                        .unwrap()
                        .chat_template
                        .supports_system_prompt()
                })
            });

            // FIXME this re-renders on any UI input :/
            log::debug!("render conversation");

            view! {
                <div class="d-flex flex-row px-4 pt-2 shadow-sm w-100">
                    <h4>
                        {title}
                    </h4>
                    <h6 class="mt-auto ms-4">
                        <span class="badge bg-secondary">
                            {move || with!(|model_id| view!{
                                <a href=format!("https://huggingface.co/{model_id}") target="_blank" class="text-white text-decoration-none">{model_id.to_string()}</a>
                            })}
                            <span class="ms-1">
                                <BootstrapIcon icon="link-45deg" />
                            </span>
                        </span>
                    </h6>
                    <div class="d-flex flex-row ms-auto pb-2">
                        <button
                            type="button"
                            class="btn btn-sm"
                            style="height: 100%;"
                            class:btn-outline-danger=move || !confirm_delete.get()
                            class:btn-danger=confirm_delete
                            on:click=delete_button_clicked
                        >
                            <BootstrapIcon icon="trash-fill" />
                        </button>
                    </div>
                </div>
                <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100">
                    <For
                        each=move || with!(|conversation| conversation.messages.clone())
                        key=|message_id| *message_id
                        children=move |message_id| {
                            view! {
                                <Message id=message_id />
                            }
                        }
                    />
                    <div class="d-flex w-100 h-0" node_ref=scroll_target></div>
                    <div class="d-flex w-100" style="min-height: 10em;"></div>
                </div>
                <form on:submit=on_submit class="p-4 shadow-lg needs-validation" novalidate>
                    <div class="collapse pb-2" id="sendMessageAdvancedContainer">
                        <ConversationParametersInputGroup
                            value=Signal::derive(move || conversation.with_untracked(|conversation| conversation.conversation_parameters.clone()))
                            on_system_prompt_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.system_prompt = value)
                            on_start_response_with_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.start_response_with = value)
                            on_temperature_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.temperature = value)
                            on_top_k_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.top_k = value)
                            on_top_p_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.top_p = value)
                            on_repetition_penalty_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.repetition_penalty = value)
                            on_token_limit_input=move |value| update_conversation.update(move |conversation| conversation.conversation_parameters.token_limit = value)
                            hide_system_prompt=hide_system_prompt_input
                        />
                    </div>
                    <div class="input-group input-group-lg mb-3">
                        <input
                            type="text"
                            class="form-control"
                            placeholder="Ask anything"
                            value=move || with!(|conversation| conversation.user_message.clone())
                            on:input=move |event| {
                                let user_message = event_target_value(&event);
                                update_conversation.update(|conversation| conversation.user_message = user_message);
                            }
                        />
                        <button class="btn btn-outline-secondary" type="submit" disabled=is_loading>
                            {move || {
                                if is_loading.get() {
                                    view! {
                                        <div class="spinner-grow spinner-grow-sm" role="status">
                                            <span class="visually-hidden">"Loading..."</span>
                                        </div>
                                    }.into_view()
                                }
                                else {
                                    view!{ <BootstrapIcon icon="send" /> }.into_view()
                                }
                            }}
                        </button>
                        <button class="btn btn-outline-secondary" type="button" data-bs-toggle="collapse" data-bs-target="#sendMessageAdvancedContainer"><BootstrapIcon icon="three-dots" /></button>
                        //<button class="btn btn-outline-secondary" type="button" on:click=|_|{}><BootstrapIcon icon="gear" /></button>
                    </div>
                </form>
            }
        }}
    }
}

#[component]
fn Message(#[prop(into)] id: MaybeSignal<MessageId>) -> impl IntoView {
    let message = Signal::derive(move || {
        let StorageSignals { read: message, .. } = use_message(id.get());
        message.get()
    });

    view! {
        {move || {
            // when the assistant replies, there is a moment where the message id is logged, but the message hasn't been created yet.
            // not sure if this is a good way to do this, but we can just ignore the message in this case.
            message.get().map(|message| {
                let is_assistant = matches!(message.role, Role::Assitant);
                let html = markdown::to_html(&message.text);

                view!{
                    <div
                        class="rounded rounded-3 w-75 mw-75 my-2 p-2 shadow-sm message-background markdown"
                        class:ms-auto=is_assistant
                        inner_html=html
                    >
                    </div>
                }
            })
        }}
    }
}

#[component]
fn ConversationParametersInputGroup(
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

#[component]
fn Settings() -> impl IntoView {
    view! {
        <div class="d-flex flex-column h-100 w-100 p-4">
            <form on:submit=|e| e.prevent_default()>
                <button type="button" class="btn btn-danger" on:click=|_| clear_storage()>"Reset"</button>
            </form>
        </div>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! { "Not found" }
}
