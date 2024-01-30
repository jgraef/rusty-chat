pub mod conversation;
pub mod conversation_parameters;
pub mod home;
pub mod settings;

use std::sync::Arc;

use chrono::Local;
use futures::{
    stream::TryStreamExt,
    FutureExt,
};
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
    AppVersion,
    ConversationId,
    Message,
    MessageId,
    Role,
    StorageSignals,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("text generation error")]
    TextGeneration(#[from] hf_textgen::Error),
}

#[derive(Clone)]
pub struct Context {
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

pub fn expect_context() -> Context {
    leptos::expect_context::<Context>()
}

pub fn push_user_message(conversation_id: ConversationId, user_message: String) {
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
                    (
                        id,
                        Signal::derive(move || with!(|conversation| conversation.title.clone())),
                        with!(|conversation| conversation.timestamp_last_interaction),
                    )
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
                ColorMode::Dark => "moon-stars-fill",
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
