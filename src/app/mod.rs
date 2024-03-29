pub mod conversation;
pub mod home;
pub mod settings;

use std::cmp::Ordering;

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
    create_trigger,
    spawn_local,
    view,
    with,
    Children,
    CollectView,
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
    Trigger,
    WriteSignal,
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
use crate::{
    config::GITHUB_PAGE,
    state::{
        use_conversation,
        use_message,
        use_storage,
        ConversationId,
        Conversations,
        Home,
        Message,
        MessageId,
        Role,
        Settings,
        StorageKey,
        StorageSignals,
    },
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
    trace: Vec<String>,
}

#[derive(Copy, Clone)]
pub struct Errors(RwSignal<Vec<ErrorMessage>>);

impl Default for Errors {
    fn default() -> Self {
        Self(create_rw_signal(vec![]))
    }
}

impl Errors {
    pub fn push(&self, error: impl std::error::Error) {
        let message = error.to_string();

        log::error!("reporting error: {message}");

        let trace = {
            let mut trace = vec![];
            let mut error: &dyn std::error::Error = &error;

            while let Some(source) = error.source() {
                trace.push(source.to_string());
                error = source;
            }

            trace
        };

        let error = ErrorMessage {
            id: Uuid::new_v4(),
            message,
            trace,
        };
        self.0.update(|errors| errors.push(error))
    }
}

#[derive(Clone)]
pub struct Context {
    pub is_loading: RwSignal<bool>,
    pub errors: Errors,
    pub settings: Signal<Settings>,
    pub update_settings: WriteSignal<Settings>,
    pub home: Signal<Home>,
    pub update_home: WriteSignal<Home>,
    pub conversations: Signal<Conversations>,
    pub update_conversations: WriteSignal<Conversations>,
    pub scroll_trigger: Trigger,
}

fn provide_context() {
    log::info!("app version: {}", *VERSION);

    let StorageSignals {
        write: update_version,
        ..
    } = use_storage(StorageKey::Version);
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

    let StorageSignals {
        read: settings,
        write: update_settings,
        ..
    } = use_storage(StorageKey::Settings);
    let StorageSignals {
        read: home,
        write: update_home,
        ..
    } = use_storage(StorageKey::Home);
    let StorageSignals {
        read: conversations,
        write: update_conversations,
        ..
    } = use_storage(StorageKey::Conversations);

    let scroll_trigger = create_trigger();

    leptos::provide_context(Context {
        is_loading: create_rw_signal(false),
        errors: Errors::default(),
        settings,
        update_settings,
        home,
        update_home,
        conversations,
        update_conversations,
        scroll_trigger,
    });
}

pub fn expect_context() -> Context {
    leptos::expect_context::<Context>()
}

pub fn push_user_message(conversation_id: ConversationId, user_message: String) {
    let Context {
        is_loading,
        errors,
        settings,
        scroll_trigger,
        ..
    } = expect_context();

    let api = settings.with_untracked(|settings| settings.api());

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

    let StorageSignals {
        write: update_conversation,
        ..
    } = use_conversation(conversation_id);

    // add message to conversation and get model_id and prompt
    let result = {
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

                let (chat_template, stream) = settings.with_untracked(|settings| {
                    let model = settings.models.get(&model_id).unwrap();
                    (model.chat_template, model.stream)
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
                    stream,
                ))
            })
            .unwrap()
    };

    scroll_trigger.notify();

    let (model_id, prompt, conversation_parameters, stream) = match result {
        Ok(x) => x,
        Err(e) => {
            errors.push(e);
            return;
        }
    };

    let mut model = api.text_generation(&model_id.0);
    let default_token_limit = stream.then_some(2000).unwrap_or(250);
    model.max_new_tokens = Some(
        conversation_parameters
            .token_limit
            .unwrap_or(default_token_limit),
    );
    model.temparature = conversation_parameters.temperature.unwrap_or(1.0);
    model.top_k = conversation_parameters.top_k;
    model.top_p = conversation_parameters.top_p;
    model.repetition_penalty = conversation_parameters.repetition_penalty;

    spawn_local(
        async move {
            is_loading.set(true);

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

            scroll_trigger.notify();

            update_conversation.update(|conversation| {
                if let Some(conversation) = conversation {
                    conversation.messages.push(message_id);
                    conversation.timestamp_last_interaction = now;
                }
                else {
                    log::warn!("conversation does not exist: {conversation_id}");
                }
            });

            if stream {
                let mut stream = model.generate_stream(&prompt).await?;

                while let Some(token) = stream.try_next().await? {
                    if token.special {
                        continue;
                    }

                    set_message.update(move |message| {
                        let message = message.as_mut().unwrap();
                        message.text.push_str(&token.text);
                        scroll_trigger.notify();
                    });
                }
            }
            else {
                let response = model.generate(&prompt).await?;

                set_message.update(move |message| {
                    let message = message.as_mut().unwrap();
                    message.text = response;
                    scroll_trigger.notify();
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
    let Context {
        errors, settings, ..
    } = expect_context();

    let mut model = settings
        .with_untracked(|settings| settings.api())
        .text_generation("NousResearch/Nous-Hermes-2-Mixtral-8x7B-DPO");
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
            let response = model.generate(&prompt).await?;

            // only use the first line.
            let mut lines = response.lines();
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

    let Context { conversations, .. } = expect_context();

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
                        </A>
                        <small class="d-flex flex-row">
                            <button type="button" class="btn py-0 px-1 m-auto" style="color: white;" on:click=move |_| toggle_theme()>
                                {move || {
                                    view!{<BootstrapIcon icon=theme_icon.get() />}
                                }}
                            </button>
                            <a href=GITHUB_PAGE target="_blank" class="py-0 px-1 m-auto" style="color: white;">
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
                                    let title = Signal::derive(move || with!(|conversation| conversation.as_ref().and_then(|conversation| conversation.title.clone())));

                                    view! {
                                        <NavLink href=format!("/conversation/{}", item.id)>
                                            <div class="text-nowrap text-truncate" style="width: 200px">
                                                {move || {
                                                    if let Some(title) = title.get() {
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
                <main class="main d-flex flex-column w-100 h-100 mw-100 mh-100 position-relative">
                    // error message
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
                                    <h5>
                                        <span class="me-2"><BootstrapIcon icon="exclamation-circle" /></span>
                                        {error.message}
                                    </h5>
                                    <ol>
                                        {
                                            error.trace
                                                .into_iter()
                                                .map(|message| view!{ <li>{message}</li> })
                                                .collect_view()
                                        }
                                    </ol>
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
                        <Route path="/*any" view=NotFound />
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div class="h-100 w-100 pt-3 px-4">
            <h1>"404 - Not found"</h1>
        </div>
    }
}
