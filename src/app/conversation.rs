use std::{
    fmt::Display,
    str::FromStr,
};

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
    Callback,
    For,
    IntoView,
    MaybeSignal,
    NodeRef,
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
use leptos_router::{
    use_navigate,
    NavigateOptions,
};
use web_sys::{
    Event,
    ScrollLogicalPosition,
    SubmitEvent,
};

use crate::{
    app::{
        expect_context,
        push_user_message,
        BootstrapIcon,
        Context,
    },
    state::{
        delete_storage,
        use_conversation,
        use_message,
        ConversationId,
        ConversationParameters,
        MessageId,
        Role,
        StorageKey,
        StorageSignals,
    },
    utils::non_empty,
};

#[component]
pub fn Conversation(#[prop(into)] id: MaybeSignal<ConversationId>) -> impl IntoView {
    let Context {
        is_loading,
        settings,
        update_conversations,
        ..
    } = expect_context();

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

            // create effect to auto-scroll
            let scroll_target = create_node_ref::<Div>();
            //let is_initial_scroll = create_rw_signal(true);

            /*create_effect(move |_| {
                conversation.with(|_| ());
                let initial = is_initial_scroll.get_untracked();
                scroll_to(scroll_target, !initial);
                is_initial_scroll.set(false);
            });*/

            let user_message_input = create_node_ref::<Input>();

            // send message

            let on_submit = move |event: SubmitEvent| {
                event.prevent_default();

                let id = id.get_untracked();

                let Some(user_message_input) = user_message_input.get_untracked() else {
                    log::error!("user_message_input missing");
                    return;
                };

                let user_message = user_message_input.value();
                if user_message.is_empty() {
                    return;
                }

                // clear message field
                user_message_input.set_value("");

                // clear message in local storage
                update_conversation.try_update(|conversation| {
                    let Some(conversation) = conversation else {
                        log::warn!("conversation gone: {id}");
                        return;
                    };
                    conversation.user_message = "".to_owned();
                });

                push_user_message(id, user_message);
            };

            let title = Signal::derive(move || {
                with!(|conversation| conversation.as_ref().and_then(|conversation| conversation.title.clone()))
            });

            let model_id = Signal::derive(move || {
                with!(|conversation| conversation.as_ref().and_then(|conversation| conversation.model_id.clone()))
            });

            let model_name = Signal::derive(move || {
                with!(|conversation, settings| {
                    conversation.as_ref().and_then(move |conversation| {
                        conversation.model_id.as_ref().and_then(move |model_id| {
                            settings.models.get(model_id)
                                .map(|model| model.display_name().to_owned())
                        })
                    })
                })
            });

            let hide_system_prompt_input = Signal::derive(move || {
                with!(|settings, model_id| {
                    let Some(model_id) = model_id else { return false };
                    !settings
                        .models
                        .get(model_id)
                        .unwrap()
                        .chat_template
                        .supports_system_prompt()
                })
            });

            let disable_send = Signal::derive(move || {
                is_loading.get() || with!(|conversation| conversation.as_ref().map(|conversation| conversation.user_message.is_empty()).unwrap_or(true))
            });

            let edit_title = create_rw_signal(false);

            log::debug!("render conversation: {}", id.get_untracked());

            // this takes a closure which updates the conversation parameters and returns a closure that only takes the new value.
            fn update_conversation_parameters<T>(
                update_conversation: WriteSignal<Option<crate::state::Conversation>>,
                update: impl FnMut(&mut ConversationParameters, T) + Clone
            ) -> impl Fn(T) {
                move |value| {
                    let mut update = update.clone();
                    update_conversation.update(move |conversation: &mut Option<crate::state::Conversation>| {
                        let Some(conversation) = conversation.as_mut() else { return; };
                        update(&mut conversation.conversation_parameters, value);
                    });
                }
            }

            let delete_conversation = move |_| {
                let id = id.get_untracked();

                log::warn!("deleting conversation: {}", id);

                // browse to home, but don't remember this page in the history.
                use_navigate()("/", NavigateOptions {
                    replace: true,
                    ..Default::default()
                });

                // remove from conversations list
                update!(|update_conversations| {
                    update_conversations.remove(&id);
                });

                // remove the conversation
                let conversation = use_conversation(id);
                let message_ids = conversation
                    .read
                    .with(|conversation| {
                        let Some(conversation) = conversation else {
                            log::warn!("conversation gone: {id}");
                            return vec![];
                        };
                        conversation.messages.clone()
                    });
                conversation.delete();

                // remove all messages
                for message_id in message_ids {
                    delete_storage(StorageKey::Message(message_id));
                }
            };

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
                                    on:click=delete_conversation
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
                                update_conversation.update(move |conversation| {
                                    let Some(conversation) = conversation else {
                                        log::warn!("conversation gone");
                                        return;
                                    };
                                    conversation.title = Some(new_title)
                                });
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
                    {move || {
                        with!(|model_id, model_name| {
                            model_id.as_ref().map(move |model_id| {
                                view!{
                                    <h6 class="mt-auto ms-4">
                                        <span class="badge bg-secondary">
                                            <a href={model_id.url()} target="_blank" class="text-white text-decoration-none">{model_name.as_ref().cloned()}</a>
                                            <span class="ms-1">
                                                <BootstrapIcon icon="link-45deg" />
                                            </span>
                                        </span>
                                    </h6>
                                }
                            })
                        })
                    }}
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
                        each=move || with!(|conversation| conversation.as_ref().map(|conversation| conversation.messages.clone()).unwrap_or_default())
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
                <div class="d-flex flex-column px-3 pt-3 shadow-lg">
                    <div class="collapse pb-2" id="sendMessageAdvancedContainer">
                        <ConversationParametersInputGroup
                            value=Signal::derive(move || conversation.with_untracked(|conversation| {
                                conversation.as_ref().map(|conversation| conversation.conversation_parameters.clone())
                                    .unwrap_or_default()
                            }))
                            on_system_prompt_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.system_prompt = value)
                            on_start_response_with_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.start_response_with = value)
                            on_temperature_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.temperature = value)
                            on_top_k_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.top_k = value)
                            on_top_p_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.top_p = value)
                            on_repetition_penalty_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.repetition_penalty = value)
                            on_token_limit_input=update_conversation_parameters(update_conversation, |params: &mut ConversationParameters, value| params.token_limit = value)
                            hide_system_prompt=hide_system_prompt_input
                        />
                    </div>
                    <form on:submit=on_submit>
                        <div class="input-group input-group-lg mb-3">
                            <input
                                type="text"
                                class="form-control"
                                placeholder="Ask anything"
                                value=move || {
                                    conversation.with_untracked(|conversation| {
                                        conversation.as_ref()
                                            .map(|conversation| conversation.user_message.clone())
                                            .unwrap_or_default()
                                    })
                                }
                                node_ref=user_message_input
                                on:input=move |event| {
                                    let user_message = event_target_value(&event);
                                    update_conversation.update(|conversation| {
                                        let Some(conversation) = conversation else { return; };
                                        conversation.user_message = user_message
                                    });
                                }
                            />
                            <button class="btn btn-outline-secondary" type="submit" disabled=disable_send>
                                {move || {
                                    if is_loading.get() {
                                        view! {
                                            <div class="spinner-border spinner-border-sm" role="status">
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
                </div>
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

/*
        on_input=move |update| update_conversation(move |conversation| {
            let Some(conversation) = conversation.as_mut() else { return; };
            update(&mut conversation.conversation_parameters);
        })
*/

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
    ) where
        T::Err: Display,
    {
        let value = event_target_value(event);
        let value = non_empty(value).map(|s| s.parse::<T>()).transpose();
        let valid = if let Err(e) = &value {
            log::debug!("parse failed: {e}");
            false
        }
        else {
            true
        };
        let value = value.ok().flatten();

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
            <span class="input-group-text">"System prompt"</span>
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
                    type="text"
                    class="form-control"
                    class:is-invalid=invalid_temperature
                    value=with!(|value| value.temperature)
                    on:input=move |event| on_input(on_temperature_input, &event, Some(invalid_temperature))
                />
            </div>
            <div class="input-group me-3">
                <span class="input-group-text">"Top K"</span>
                <input
                    type="text"
                    class="form-control"
                    class:is-invalid=invalid_top_k
                    value=with!(|value| value.top_k)
                    on:input=move |event| on_input(on_top_k_input, &event, Some(invalid_top_k))
                />
            </div>
            <div class="input-group me-3">
                <span class="input-group-text">"Top P"</span>
                <input
                    type="text"
                    class="form-control"
                    class:is-invalid=invalid_top_p
                    value=with!(|value| value.top_p)
                    on:input=move |event| on_input(on_top_p_input, &event, Some(invalid_top_p)) />
            </div>
            <div class="input-group me-3">
                <span class="input-group-text">"Repetition penalty"</span>
                <input
                    type="text"
                    class="form-control"
                    class:is-invalid=invalid_repetition_penalty
                    value=with!(|value| value.repetition_penalty)
                    on:input=move |event| on_input(on_repetition_penalty_input, &event, Some(invalid_repetition_penalty)) />
            </div>
            <div class="input-group">
                <span class="input-group-text">"Token limit"</span>
                <input
                    type="text"
                    class="form-control"
                    class:is-invalid=invalid_token_limit
                    value={with!(|value| value.token_limit)}
                    on:input=move |event| on_input(on_token_limit_input, &event, Some(invalid_token_limit))
                />
            </div>
        </div>
    }
}
