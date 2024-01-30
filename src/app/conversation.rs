use leptos::{
    component,
    create_node_ref,
    create_rw_signal,
    event_target_value,
    html::Div,
    update,
    view,
    with,
    For,
    IntoView,
    MaybeSignal,
    NodeRef,
    Signal,
    SignalGet,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWith,
    SignalWithUntracked,
};
use leptos_router::use_navigate;
use web_sys::{
    ScrollLogicalPosition,
    SubmitEvent,
};

use crate::{
    app::{
        conversation_parameters::ConversationParametersInputGroup,
        expect_context,
        push_user_message,
        BootstrapIcon,
        Context,
    },
    state::{
        delete_storage,
        use_conversation,
        use_conversations,
        use_message,
        use_settings,
        ConversationId,
        MessageId,
        Role,
        StorageKey,
        StorageSignals,
    },
};

#[component]
pub fn Conversation(#[prop(into)] id: MaybeSignal<ConversationId>) -> impl IntoView {
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
