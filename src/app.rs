use std::sync::Arc;

use chrono::Local;
use futures::{
    stream::TryStreamExt,
    FutureExt,
};
use leptos::{
    component,
    create_memo,
    create_node_ref,
    create_signal,
    ev::SubmitEvent,
    event_target_value,
    html::{
        Input,
        Main,
    },
    spawn_local_with_current_owner,
    view,
    Children,
    For,
    IntoView,
    NodeRef,
    ReadSignal,
    Signal,
    SignalGet,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWith,
    SignalWithUntracked,
    WriteSignal,
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
use leptos_use::storage::{
    use_local_storage,
    JsonCodec,
};
use uuid::Uuid;

use crate::state::{
    use_message,
    use_storage,
    Conversation,
    ConversationId,
    HyperParameters,
    Message,
    MessageId,
    ModelId,
    Role,
    State,
    StorageKey,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("text generation error")]
    TextGeneration(#[from] hf_textgen::Error),
}

#[derive(Clone)]
struct Context {
    pub api: Arc<hf_textgen::Api>,
    pub state: Signal<State>,
    pub update_state: WriteSignal<State>,
    pub is_loading: ReadSignal<bool>,
    pub set_loading: WriteSignal<bool>,
}

fn provide_context() {
    let (state, update_state, _) = use_storage(StorageKey::State);
    let (is_loading, set_loading) = create_signal(false);

    leptos::provide_context(Context {
        api: Arc::new(hf_textgen::Api::default()),
        state,
        update_state,
        is_loading,
        set_loading,
    });
}

fn expect_context() -> Context {
    leptos::expect_context::<Context>()
}

#[component]
pub fn BootstrapIcon(#[prop(into)] icon: String) -> impl IntoView {
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
    provide_context();
    let Context {
        state, ..
    } = expect_context();

    view! {
        <Router>
            <div class="d-flex flex-row" style="height: 100vh; width: 100%">
                <nav class="d-flex flex-column flex-shrink-0 p-3 text-white bg-dark shadow-lg" style="width: 280px;">
                    <A class="d-flex align-items-center mb-3 mb-md-0 me-md-auto text-white text-decoration-none" href="/">
                        <span class="fs-4">"🦀 RustyChat"</span>
                    </A>
                    <hr />
                    <ul class="nav nav-pills flex-column mb-auto">
                        <For
                            each=move || state.with(|state| state.conversations.values().cloned().collect::<Vec<_>>())
                            key=|metadata| metadata.id
                            children=move |metadata| {
                                let id = metadata.id;
                                view! {
                                    <NavLink href=format!("/conversation/{id}")>
                                        {
                                            if let Some(title) = metadata.title {
                                                view!{{title}}.into_view()
                                            }
                                            else {
                                                view!{
                                                    <span class="me-2"><BootstrapIcon icon="question-lg" /></span>
                                                    "Untitled"
                                                }.into_view()
                                            }
                                        }
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
                <main class="w-100 p-0">
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

/*fn scroll_main_panel_to_bottom() {
    let Context { main_panel, .. } = expect_context();
    let main_panel = main_panel.get_untracked().unwrap();
    main_panel.set_scroll_top(main_panel.scroll_height());
}*/

fn push_user_message(conversation_id: ConversationId, user_message: String) {
    let Context {
        api,

        update_state,

        set_loading,
        ..
    } = expect_context();

    let message_id = MessageId::new();
    let now = Local::now();

    // create and store message
    let (_, set_message, _) = use_message(message_id);
    set_message.set(Some(Message {
        id: message_id,
        role: Role::User,
        text: user_message,
        timestamp: now,
    }));

    // add message to conversation and get model_id and prompt
    let (model_id, prompt) = update_state
        .try_update(move |state| {
            let conversation = state.conversations.get_mut(&conversation_id).unwrap();

            conversation.messages.push(message_id);
            conversation.timestamp_last_interaction = now;

            let model = state.models.get(&conversation.model_id).unwrap();

            let messages = conversation
                .messages
                .iter()
                .map(|message_id| use_message(*message_id).0.get_untracked().unwrap())
                .collect::<Vec<_>>();

            let prompt = model.chat_template.generate_prompt(
                conversation.system_prompt.as_ref().map(|s| s.as_str()),
                &messages,
            );

            (conversation.model_id.clone(), prompt)
        })
        .unwrap();

    // get model's chat template

    let mut model = api.text_generation(&model_id.0);
    model.max_new_tokens = Some(2048);

    spawn_local_with_current_owner(
        async move {
            set_loading.set(true);

            log::debug!("prompt: {prompt}");
            let mut stream = model.generate(&prompt).await?;

            let message_id = MessageId::new();
            let now = Local::now();

            let (_, set_message, _) = use_message(message_id);

            update_state.update(|state| {
                let conversation = state.conversations.get_mut(&conversation_id).unwrap();
                conversation.messages.push(message_id);
                conversation.timestamp_last_interaction = now;
            });

            while let Some(token) = stream.try_next().await? {
                log::debug!("token: {token:?}");
                if token.special {
                    continue;
                }

                set_message.update(|message| {
                    let message = message.get_or_insert_with(|| {
                        Message {
                            id: message_id,
                            role: Role::Assitant,
                            text: "".to_owned(),
                            timestamp: now,
                        }
                    });

                    let mut text: &str = &token.text;
                    if message.text.is_empty() {
                        text = text.trim_start();
                    }

                    message.text.push_str(text);
                });
            }
            Ok(())
        }
        .map(move |result: Result<(), Error>| {
            if let Err(e) = result {
                log::error!("{e}");
            }
            log::debug!("response stream finished");
            set_loading.set(false);
        }),
    )
    .expect("future failed");
}

#[component]
fn Home() -> impl IntoView {
    let user_message_input = create_node_ref::<Input>();

    let Context {
        state,
        update_state,
        ..
    } = expect_context();
    let current_model = Signal::derive(move || state.with(|state| state.current_model.clone()));

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let user_message = user_message_input.get().unwrap().value();

        let now = Local::now();

        let conversation_id = ConversationId::new();
        let conversation = Conversation {
            id: conversation_id,
            model_id: current_model.get_untracked(),
            hyper_parameters: HyperParameters { temperature: 1.0 },
            system_prompt: None,
            title: None,
            timestamp_started: now,
            timestamp_last_interaction: now,
            messages: vec![],
        };

        update_state.update(|state| {
            state.conversations.insert(conversation_id, conversation);
        });

        push_user_message(conversation_id, user_message);

        use_navigate()(
            &format!("/conversation/{conversation_id}"),
            Default::default(),
        );
    };

    let model_selected = move |e| {
        let model_id = ModelId(event_target_value(&e));
        log::debug!("model selected: {model_id}");
        update_state.update(move |state| {
            state.current_model = model_id;
        });
    };

    view! {
        <div class="d-flex flex-column h-100 w-100 p-4">
            <div class="mb-auto">
                // TODO: say hello to the user
            </div>
            <form on:submit=on_submit class="p-2">
                <div class="collapse p-4" id="startChatAdvancedContainer">
                    <div class="input-group mb-3">
                        <span class="input-group-text">"System Prompt"</span>
                        <textarea class="form-control" id="systemPromptTextarea" rows="3"></textarea>
                    </div>
                    <div class="mb-3">
                        <label class="form-label">"Hyperparameters"</label>
                        <div class="input-group mb-3">
                            <span class="input-group-text">"Temperature"</span>
                            <input type="text" class="form-control" value="1.0" />
                        </div>
                    </div>
                </div>
                <div class="input-group mb-3">
                    <span class="input-group-text">"Model"</span>
                    <select class="form-select" aria-label="Select a model to chat with" on:change=model_selected>
                        <For
                            each=move || state.with(|state| state.models.keys().cloned().collect::<Vec<_>>())
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
                    <input type="text" class="form-control" placeholder="Ask anything" node_ref=user_message_input value="Write a short poem about AI." />
                    <button class="btn btn-outline-secondary" type="submit"><BootstrapIcon icon="send" /></button>
                    <button class="btn btn-outline-secondary" type="button" data-bs-toggle="collapse" data-bs-target="#startChatAdvancedContainer"><BootstrapIcon icon="three-dots" /></button>
                </div>
            </form>
        </div>
    }
}

#[component]
fn Conversation(#[prop(into)] id: Signal<ConversationId>) -> impl IntoView {
    let Context {
        state, is_loading, ..
    } = expect_context();

    let user_message_input = create_node_ref::<Input>();

    let (role, set_role) = create_signal(Role::User);

    let conversation = Signal::derive(move || {
        state
            .get()
            .conversations
            .get(&id.get())
            .expect("missing conversation metadata")
            .clone()
    });

    let on_submit = move |event: SubmitEvent| {
        event.prevent_default();
        let user_message_input = user_message_input.get().unwrap();
        let user_message = user_message_input.value();
        user_message_input.set_value("");
        log::debug!("user_message: {user_message}");

        let id = conversation.with_untracked(|conversation| conversation.id);
        push_user_message(id, user_message);
    };

    let messages = create_memo(|_| {
        // todo memo the actual messages in order
        // then render for loop over that. i think that's the bug!
    });

    view! {
        /*<div class="d-flex flex-row">
            <h6>
                {move || metadata.with(|metadata| metadata.title.clone())}
                <span class="badge rounded-pill bg-dark">{move || metadata.with(|metadata| metadata.model_id.to_string())}</span>
            </h6>
        </div>*/
        <div class="d-flex flex-column overflow-scroll mb-auto p-4">
            <For
                each=move || conversation.with(|conversation| conversation.messages.clone())
                key=|message_id| *message_id
                children=move |message_id| {
                    log::debug!("render message: {message_id}");
                    let (message, _, _) = use_message(message_id);

                    view! {
                        {move || {
                            // when the assistant replies, there is a moment where the message id is logged, but the message hasn't been created yet.
                            // not sure if this is a good way to do this, but we can just ignore the message in this case.
                            message.get().map(|message| {
                                let is_assistant = matches!(message.role, Role::Assitant);
                                let html = markdown::to_html(&message.text);

                                view!{
                                    <div
                                        class="rounded rounded-3 bg-gradient bg-light w-75 my-2 p-2 shadow-sm"
                                        class:ms-auto=is_assistant

                                    >
                                        <p inner_html=html></p>
                                    </div>
                                }
                            })
                        }}
                    }
                }
            />
        </div>
        <form on:submit=on_submit class="px-4 pb-4 pt-2 shadow">
            <div class="collapse p-4" id="sendMessageAdvancedContainer">
                <div class="input-group">
                    <span class="input-group-text">"Role"</span>
                    <div class="btn-group">
                        <button
                            type="button"
                            class="btn btn-primary btn-outline-secondary"
                            class:btn-primary=move || role.get() == Role::User
                            class:btn-light=move || role.get() != Role::User
                            class:btn-outline-dark=move || role.get() == Role::User
                            class:btn-outline-secondary=move || role.get() != Role::User
                            on:click=move |_| set_role(Role::User)
                        >
                            "User"
                        </button>
                        <button
                            type="button"
                            class="btn btn-light"
                            class:btn-primary=move || role.get() == Role::Assitant
                            class:btn-light=move || role.get() != Role::Assitant
                            class:btn-outline-dark=move || role.get() == Role::Assitant
                            class:btn-outline-secondary=move || role.get() != Role::Assitant
                            on:click=move |_| set_role(Role::Assitant)
                        >
                            "Assistant"
                        </button>
                    </div>
                </div>
            </div>
            <div class="input-group input-group-lg mb-3">
                <input type="text" class="form-control" placeholder="Ask anything" node_ref=user_message_input />
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
}

#[component]
fn Settings() -> impl IntoView {
    let reset = |_| {
        let (_, _, clear_state) = use_local_storage::<State, JsonCodec>("state");
        clear_state();
    };

    view! {
        <form on:submit=|e| e.prevent_default()>
            <button type="button" class="btn btn-danger" on:click=reset>"Reset"</button>
        </form>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! { "Not found" }
}
