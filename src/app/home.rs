use chrono::Local;
use leptos::{
    component,
    create_node_ref,
    ev::SubmitEvent,
    event_target_value,
    html::Input,
    view,
    with,
    For,
    IntoView,
    Signal,
    SignalGetUntracked,
    SignalSet,
    SignalUpdate,
    SignalWith,
    SignalWithUntracked,
};
use leptos_router::use_navigate;

use super::{
    conversation_parameters::ConversationParametersInputGroup,
    push_user_message,
    request_conversation_title,
    BootstrapIcon,
};
use crate::state::{
    use_conversation,
    use_conversations,
    use_home,
    use_settings,
    Conversation,
    ConversationId,
    ModelId,
    StorageSignals,
};

#[component]
pub fn Home() -> impl IntoView {
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