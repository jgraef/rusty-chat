pub mod conversation;
pub mod home;
pub mod settings;

use std::{
    cmp::Ordering,
    fmt::Display,
};

use chrono::{
    DateTime,
    Local,
};
use futures::{
    stream::TryStreamExt,
    FutureExt,
};
use lazy_static::lazy_static;
use leptos::{
    component,
    create_memo,
    create_rw_signal,
    spawn_local,
    view,
    with,
    Children,
    DynAttrs,
    For,
    IntoView,
    Oco,
    RwSignal,
    Signal,
    SignalGet,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWith,
    SignalWithUntracked,
};
use leptos_meta::{
    provide_meta_context,
    Html,
};
use leptos_router::{
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
use semver::Version;
use uuid::Uuid;

use self::{
    conversation::Conversation,
    home::Home,
    settings::SettingsRoutes,
};
use crate::state::{
    use_conversation,
    use_conversations,
    use_message,
    use_settings,
    use_version,
    ConversationId,
    Message,
    MessageId,
    Role,
    StorageSignals,
};

lazy_static! {
    pub static ref VERSION: Version = std::env!("CARGO_PKG_VERSION")
        .parse()
        .expect("invalid version");
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Hugging Face API error")]
    HfApiError(#[from] hf_textgen::Error),
    #[error("Conversation not found: {0}")]
    ConversationNotFound(ConversationId),
    #[error("Model ID not set")]
    ModelIdNotSet,
}

#[derive(Clone, Debug)]
pub struct ErrorMessage {
    id: Uuid,
    message: String,
}

#[derive(Copy, Clone)]
pub struct Errors(RwSignal<Vec<ErrorMessage>>);

impl Default for Errors {
    fn default() -> Self {
        Self(create_rw_signal(vec![]))
    }
}

impl Errors {
    pub fn push(&self, message: impl Display) {
        log::error!("{message}");
        let error = ErrorMessage {
            id: Uuid::new_v4(),
            message: message.to_string(),
        };
        self.0.update(|errors| errors.push(error))
    }
}

#[derive(Clone)]
pub struct Context {
    pub api: hf_textgen::Api,
    pub is_loading: RwSignal<bool>,
    pub errors: Errors,
}

fn provide_context() {
    log::info!("app version: {}", *VERSION);

    let StorageSignals {
        write: update_version,
        ..
    } = use_version();
    update_version.try_update(|storage_version| {
        log::info!("storage version: {:?}", storage_version);

        if let Some(storage_version) = storage_version {
            match VERSION.cmp(&storage_version) {
                Ordering::Less => {
                    log::error!("version error: storage > app");
                    panic!("version error");
                }
                Ordering::Equal => {}
                Ordering::Greater => {
                    todo!("migrate storage");
                }
            }
        }
        else {
            *storage_version = Some(VERSION.clone());
        }
    });

    leptos::provide_context(Context {
        api: hf_textgen::Api::default(),
        is_loading: create_rw_signal(false),
        errors: Errors::default(),
    });
}

pub fn expect_context() -> Context {
    leptos::expect_context::<Context>()
}

pub fn push_user_message(conversation_id: ConversationId, user_message: String) {
    let Context {
        api,
        is_loading,
        errors,
        ..
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
    let result = {
        let StorageSignals {
            write: update_conversation,
            ..
        } = use_conversation(conversation_id);
        let StorageSignals { read: settings, .. } = use_settings();

        update_conversation
            .try_update(move |conversation| {
                let conversation = conversation
                    .as_mut()
                    .ok_or_else(|| Error::ConversationNotFound(conversation_id))?;

                let model_id = conversation
                    .model_id
                    .clone()
                    .ok_or_else(|| Error::ModelIdNotSet)?;

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

                Ok::<_, Error>((
                    model_id,
                    prompt,
                    conversation.conversation_parameters.clone(),
                ))
            })
            .unwrap()
    };

    let (model_id, prompt, conversation_parameters) = match result {
        Ok(x) => x,
        Err(e) => {
            errors.push(e);
            return;
        }
    };

    let mut model = api.text_generation(&model_id.0);
    model.max_new_tokens = Some(conversation_parameters.token_limit.unwrap_or(2000));
    model.temparature = conversation_parameters.temperature.unwrap_or(1.0);
    model.top_k = conversation_parameters.top_k;
    model.top_p = conversation_parameters.top_p;
    model.repetition_penalty = conversation_parameters.repetition_penalty;

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
                text: conversation_parameters
                    .start_response_with
                    .unwrap_or_default(),
                timestamp: now,
            }));

            let StorageSignals {
                write: set_conversation,
                ..
            } = use_conversation(conversation_id);
            set_conversation.update(|conversation| {
                if let Some(conversation) = conversation {
                    conversation.messages.push(message_id);
                    conversation.timestamp_last_interaction = now;
                }
                else {
                    log::warn!("conversation does not exist: {conversation_id}");
                }
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
                log::error!("response stream failed: {e}");
                errors.push(e);
            }
            log::debug!("response stream finished");
            is_loading.set(false);
        }),
    );
}

fn request_conversation_title(conversation_id: ConversationId, user_message: &str) {
    let Context { api, errors, .. } = expect_context();

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
                if let Some(conversation) = conversation {
                    conversation.title = Some(title);
                }
                else {
                    log::warn!("conversation does not exist: {conversation_id}");
                }
            });

            Ok(())
        }
        .map(move |result: Result<(), Error>| {
            if let Err(e) = result {
                log::error!("title generation failed: {e}");
                errors.push(e);
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

    #[derive(Copy, Clone, Debug, PartialEq)]
    struct Item {
        timestamp: DateTime<Local>,
        id: ConversationId,
    }

    let sorted_items = create_memo(move |_| {
        with!(|conversations| {
            let mut sorted_items = vec![];

            for id in conversations {
                let StorageSignals {
                    read: conversation, ..
                } = use_conversation(*id);
                let Some(timestamp) = with!(|conversation| {
                    conversation
                        .as_ref()
                        .map(|conversation| conversation.timestamp_last_interaction)
                })
                else {
                    log::warn!("dangling conversation entry: {id}");
                    continue;
                };
                sorted_items.push(Item { id: *id, timestamp });
            }

            sorted_items.sort_by_cached_key(|item| item.timestamp);
            sorted_items.reverse();

            sorted_items
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
                ColorMode::Dark => "moon-stars-fill",
                _ => "sun-fill",
            }
        });
        (bs_theme, toggle_theme, theme_icon)
    };

    let Context { errors, .. } = expect_context();

    view! {
        <Html
            attr:data-bs-theme=bs_theme
        />
        <Router>
            <div class="d-flex flex-row" style="height: 100vh; width: 100%">
                <nav class="d-flex flex-column flex-shrink-0 p-3 text-white shadow-lg sidebar">
                    <div class="d-flex flex-row">
                        <A class="d-flex mb-3 mb-md-0 me-md-auto text-white text-decoration-none" href="/">
                            <span class="fs-4">"🦀 RustyChat"</span>
                            <span class="badge bg-dark mt-auto ms-1">"beta"</span>
                        </A>
                        <small class="d-flex flex-row">
                            <button type="button" class="btn py-0 px-1 m-auto" style="color: white;" on:click=move |_| toggle_theme()>
                                {move || {
                                    view!{<BootstrapIcon icon=theme_icon.get() />}
                                }}
                            </button>
                            <a href="https://github.com/jgraef/rusty-chat" target="_blank" class="py-0 px-1 m-auto" style="color: white;">
                                <BootstrapIcon icon="github" />
                            </a>
                        </small>
                    </div>
                    <hr />
                    <div class="d-flex flex-column flex-grow-1 overflow-y-scroll">
                        <ul class="d-flex flex-column nav nav-pills mb-auto">
                            <For
                                each=sorted_items
                                key=|item| item.id
                                children=move |item| {
                                    // note: i can't make this work, if we put the title signal into the memo.
                                    let StorageSignals { read: conversation, .. } = use_conversation(item.id);
                                    let title = Signal::derive(move || with!(|conversation| conversation.as_ref().map(|conversation| conversation.title.clone())));

                                    view! {
                                        <NavLink href=format!("/conversation/{}", item.id)>
                                            <div class="text-nowrap text-truncate" style="width: 200px">
                                                {move || {
                                                    let title = title.get();
                                                    if let Some(title) = title {
                                                        view!{{title}}.into_view()
                                                    }
                                                    else {
                                                        view!{
                                                            <span class="me-2"><BootstrapIcon icon="question-lg" /></span>
                                                            "Untitled"
                                                        }.into_view()
                                                    }
                                                }}
                                            </div>
                                        </NavLink>
                                    }
                                }
                            />
                        </ul>
                    </div>
                    <hr />
                    <ul class="nav nav-pills flex-column">
                        <NavLink href="/settings">
                            <span class="me-2"><BootstrapIcon icon="gear" /></span>
                            "Settings"
                        </NavLink>
                    </ul>
                </nav>
                <main class="w-100 p-0 main">
                    <div class="d-flex flex-column flex-grow-1 h-100 position-relative" style="max-height: 100vh">
                        <div class="z-1 position-absolute top-0 start-50 translate-middle-x w-50">
                            <div
                                class="alert alert-danger alert-dismissible fade show mt-4"
                                class:visually-hidden=move || errors.0.with(|errors| errors.is_empty())
                                role="alert"
                            >
                                <h5>"Error"</h5>
                                <For
                                    each=move || errors.0.get()
                                    key=|error| error.id
                                    children=|error| view!{
                                        <hr />
                                        <p>
                                            <span class="me-2"><BootstrapIcon icon="exclamation-circle" /></span>
                                            {error.message}
                                        </p>
                                    }
                                />
                                <button
                                    type="button"
                                    class="btn-close"
                                    aria-label="Close"
                                    on:click=move |_| errors.0.update(|errors| errors.clear())
                                ></button>
                            </div>
                        </div>
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
                            <SettingsRoutes />
                            <Route path="" view=NotFound />
                        </Routes>
                    </div>
                </main>
            </div>
        </Router>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! { "Not found" }
}
