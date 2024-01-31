use leptos::{
    component,
    create_effect,
    create_node_ref,
    create_rw_signal,
    event_target_value,
    html::{
        Div,
        Input,
    },
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
    utils::non_empty,
};

#[component]
pub fn Conversation(#[prop(into)] id: MaybeSignal<ConversationId>) -> impl IntoView {
    let Context { is_loading, .. } = expect_context();

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

                let Some((user_message, id)) = update_conversation.try_update(|conversation| {
                    let user_message = non_empty(std::mem::replace(&mut conversation.user_message, Default::default()))?;
                    Some((user_message, conversation.id))
                }).flatten()
                else {
                    return;
                };

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

            let disable_send = Signal::derive(move || {
                is_loading.get() || with!(|conversation| conversation.user_message.is_empty())
            });

            let edit_title = create_rw_signal(false);

            log::debug!("render conversation: {}", id.get_untracked());

            view! {
                // delete modal
                <div class="modal fade" id="conversation_delete_modal_modal" tabindex="-1">
                    <div class="modal-dialog">
                        <div class="modal-content">
                            <div class="modal-header">
                                <h5 class="modal-title">"Delete conversation"</h5>
                                <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
                            </div>
                            <div class="modal-body">
                                <p>"Confirm to delete this conversation."</p>
                            </div>
                            <div class="modal-footer">
                                <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">"Cancel"</button>
                                <button
                                    type="button"
                                    class="btn btn-danger"
                                    data-bs-dismiss="modal"
                                    on:click=move |_| {
                                        let id = id.get_untracked();

                                        log::warn!("deleting conversation: {}", id);

                                        use_navigate()("/", Default::default());

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
                                >
                                    "Delete"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                // header
                <div class="d-flex flex-row px-4 pt-3 shadow-sm w-100">
                    <div class="d-flex flex-row">
                        {move || if edit_title.get() {
                            let edit_title_input = create_node_ref::<Input>();

                            create_effect(move |_| {
                                log::debug!("focus");
                                edit_title_input.get().map(|elem| elem.focus().unwrap());
                            });

                            let set_title = move || {
                                let Some(edit_title_input) = edit_title_input.get() else { return; };
                                let Some(new_title) = non_empty(edit_title_input.value()) else { return; };
                                update_conversation.update(move |conversation| conversation.title = Some(new_title));
                                edit_title.set(false);
                            };

                            view!{
                                <form on:submit=move |e: SubmitEvent| {
                                    e.prevent_default();
                                    set_title();
                                }>
                                    <input
                                        type="text"
                                        class="form-control"
                                        placeholder="Conversation title"
                                        value=title
                                        size=move || with!(|title| title.as_ref().map(|s| s.len()))
                                        node_ref=edit_title_input
                                        on:focusout=move |_| set_title()
                                    />
                                </form>
                            }.into_view()
                        }
                        else {
                            view!{
                                <h4>
                                    {title}
                                </h4>
                                <span
                                    href="#"
                                    class="ms-1 mt-1 link-secondary"
                                    style="cursor: pointer;"
                                    on:click=move |_| {
                                        edit_title.set(true);
                                        //edit_title_input.get_untracked().unwrap().focus();
                                    }
                                >
                                    <BootstrapIcon icon="pencil-square" />
                                </span>
                            }.into_view()
                        }}
                    </div>
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
                            class="btn btn-sm btn-outline-danger"
                            style="height: 100%;"
                            data-bs-toggle="modal"
                            data-bs-target="#conversation_delete_modal_modal"
                        >
                            <BootstrapIcon icon="trash-fill" />
                        </button>
                    </div>
                </div>

                // messages
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

                // message form
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
                        <button class="btn btn-outline-secondary" type="submit" disabled=disable_send>
                            {move || {
                                if is_loading.get() {
                                    view! {
                                        <div class="spinner-grow spinner-grow-sm" role="status">
                                            <span class="visually-hidden">"Generating..."</span>
                                        </div>
                                    }.into_view()
                                }
                                else {
                                    view!{ <BootstrapIcon icon="send" /> }.into_view()
                                }
                            }}
                        </button>
                        <button class="btn btn-outline-secondary" type="button" data-bs-toggle="collapse" data-bs-target="#sendMessageAdvancedContainer"><BootstrapIcon icon="three-dots" /></button>
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
                        class="rounded rounded-3 w-75 mw-75 my-2 p-2 shadow-sm message markdown"
                        class:ms-auto=is_assistant
                        inner_html=html
                    >
                    </div>
                }
            })
        }}
    }
}
