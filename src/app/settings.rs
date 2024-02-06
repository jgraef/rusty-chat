use futures::FutureExt;
use hf_textgen::ModelState;
use leptos::{
    component,
    create_memo,
    create_node_ref,
    create_rw_signal,
    event_target_checked,
    event_target_value,
    html::{
        Input,
        Select,
    },
    spawn_local,
    view,
    with,
    Children,
    For,
    IntoView,
    SignalGet,
    SignalSet,
    SignalUpdate,
    SignalWithUntracked,
};
use leptos_router::{
    Outlet,
    Redirect,
    Route,
    ToHref,
    A,
};
use leptos_use::use_debounce_fn_with_arg_and_options;
use strum::{
    EnumIs,
    EnumMessage,
    VariantArray,
};
use web_sys::Event;

use super::{
    BootstrapIcon,
    Error,
};
use crate::{
    app::{
        expect_context,
        Context,
    },
    state::{
        clear_storage,
        ChatTemplate,
        Model,
        ModelId,
    },
    utils::non_empty,
};

#[component(transparent)]
pub fn SettingsRoutes() -> impl IntoView {
    view! {
        <Route path="/settings" view=Settings>
            <Route path="general" view=GeneralTab />
            <Route path="backends" view=BackendsTab />
            <Route path="models" view=ModelsTab />
            <Route path="debug" view=DebugTab />
            <Route path="" view=|| view!{ <Redirect path="/settings/general" /> } />
        </Route>
    }
}

#[component]
pub fn Tab<H: ToHref + 'static>(href: H, children: Children) -> impl IntoView {
    view! {
        <li class="nav-item">
            <A href={href} active_class="active" class="nav-link">
                {children()}
            </A>
        </li>
    }
}

#[component]
fn Settings() -> impl IntoView {
    let Context { settings, .. } = expect_context();

    view! {
        <div class="d-flex flex-row px-4 pt-3 w-100">
            <h4>
                <span class="me-2"><BootstrapIcon icon="gear-fill" /></span>
                Settings
            </h4>
        </div>
        <ul class="nav nav-tabs px-4 mt-2">
            <Tab href="/settings/general">"General"</Tab>
            <Tab href="/settings/models">"Models"</Tab>
            {move || {
                with!(|settings| settings.debug_mode)
                    .then(|| view!{
                        <Tab href="/settings/backends">"Backends"</Tab>
                        <Tab href="/settings/debug">"Debug"</Tab>
                    })
            }}
        </ul>
        <Outlet />
    }
}

#[component]
fn GeneralTab() -> impl IntoView {
    let Context {
        settings,
        update_settings,
        ..
    } = expect_context();

    #[derive(Clone, Copy, Debug, EnumMessage, EnumIs)]
    enum HfTokenState {
        #[strum(
            message = "Enter your Hugging Face API token. This increases your API rate limit."
        )]
        None,
        #[strum(message = "Hugging Face API token")]
        Valid,
        #[strum(message = "The entered API token is invalid")]
        Invalid,
    }

    impl HfTokenState {
        pub fn new(hf_token: &Option<String>) -> Self {
            if let Some(value) = &hf_token {
                if value.starts_with("hf_") && value.len() > 3 {
                    Self::Valid
                }
                else {
                    Self::Invalid
                }
            }
            else {
                Self::None
            }
        }
    }

    let hf_token_state =
        settings.with_untracked(|settings| create_rw_signal(HfTokenState::new(&settings.hf_token)));

    view! {
        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100 w-75 mx-auto">
            <div class="mb-3">
                <div class="form-floating">
                    <input
                        type="password"
                        class="form-control"
                        class:is-valid=move || hf_token_state.get().is_valid()
                        class:is-invalid=move || hf_token_state.get().is_invalid()
                        id="general_hf_token"
                        aria-label="Enter your Hugging Face token"
                        value=move || with!(|settings| settings.hf_token.clone())
                        on:input=move |event| {
                            let hf_token = non_empty(event_target_value(&event));
                            let new_state = HfTokenState::new(&hf_token);
                            let set_token = new_state.is_valid().then_some(hf_token).flatten();

                            update_settings.update(move |settings| {
                                settings.hf_token = set_token;
                            });
                        }
                    />
                    <label
                        for="hf_token_input"
                        class:text-success-emphasis=move || hf_token_state.get().is_valid()
                        class:text-danger-emphasis=move || hf_token_state.get().is_invalid()
                    >
                        {move || hf_token_state.get().get_message()}
                    </label>
                </div>
                <div class="form-text">"Your token is stored in your browser and is only sent to the Hugging Face API."</div>
            </div>
        </div>
    }
}

#[component]
fn BackendsTab() -> impl IntoView {
    view! {
        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100 w-75 mx-auto">
            "Backends tab"
        </div>
    }
}

#[component]
fn ModelsTab() -> impl IntoView {
    let Context {
        errors,
        settings,
        update_settings,
        ..
    } = expect_context();

    #[derive(Debug, EnumIs)]
    enum SelectedModel {
        Edit(ModelId),
        New,
    }

    impl SelectedModel {
        fn is_model(&self, model_id: &ModelId) -> bool {
            match self {
                Self::Edit(id) => id == model_id,
                _ => false,
            }
        }

        fn get_model_id(&self) -> Option<&ModelId> {
            match self {
                SelectedModel::Edit(model_id) => Some(model_id),
                SelectedModel::New => None,
            }
        }
    }

    #[derive(Copy, Clone, Debug, EnumMessage)]
    enum ModelIdInvalidReason {
        #[strum(message = "Enter a Hugging Face model ID")]
        Empty,
        #[strum(message = "This model already exists")]
        AlreadyExists,
        #[strum(message = "Model not found")]
        NotFound,
        #[strum(message = "Model not available in the free API")]
        NotLoadable,
        #[strum(message = "Inference test failed")]
        InferenceFailed,
    }

    #[derive(Copy, Clone, Debug, EnumIs)]
    enum ModelIdState {
        Checking,
        Valid,
        Invalid { reason: ModelIdInvalidReason },
    }

    impl Default for ModelIdState {
        fn default() -> Self {
            Self::Invalid {
                reason: ModelIdInvalidReason::Empty,
            }
        }
    }

    impl ModelIdState {
        fn is_valid_model_id(&self) -> bool {
            match self {
                Self::Checking | Self::Valid => true,
                Self::Invalid { reason } => {
                    match reason {
                        ModelIdInvalidReason::AlreadyExists
                        | ModelIdInvalidReason::NotLoadable
                        | ModelIdInvalidReason::InferenceFailed => true,
                        _ => false,
                    }
                }
            }
        }
    }

    let selected_model = create_rw_signal(SelectedModel::New);
    let model_name_invalid = create_rw_signal(true);
    let model_id_state = create_rw_signal(ModelIdState::default());
    let model_search_results = create_rw_signal(vec![]);
    let model_name_input_field = create_node_ref::<Input>();
    let model_id_input_field = create_node_ref::<Input>();
    let model_chat_template_input_field = create_node_ref::<Select>();
    let model_stream_input_field = create_node_ref::<Input>();
    let changes_saved = create_rw_signal(false);

    let check_model = {
        let api = settings.with_untracked(|settings| settings.api());

        move |model_id: ModelId| {
            async move {
                model_id_state.set(ModelIdState::Checking);

                // check if this is the model that we're editing

                let is_selected_model = selected_model
                    .with_untracked(|selected_model| selected_model.is_model(&model_id));
                if is_selected_model {
                    model_id_state.set(ModelIdState::Valid);
                    return Ok(());
                }

                // check if we already have a model with that id

                let model_already_exists =
                    settings.with_untracked(|settings| settings.models.get(&model_id).is_some());
                if model_already_exists {
                    model_id_state.set(ModelIdState::Invalid {
                        reason: ModelIdInvalidReason::AlreadyExists,
                    });
                    return Ok(());
                }

                // check the status endpoint for whether the model is loadable

                let mut model = api.text_generation(&model_id.0);

                let is_loadable = model
                    .status()
                    .await
                    .map_err(|error| log::error!("model status failed: {error}"))
                    .ok()
                    .map(|status| status.state == ModelState::Loadable)
                    .unwrap_or_default();

                if is_loadable {
                    // some models are in theory loadable but will always fail to do inference. so
                    // we check if we can do inference

                    log::debug!("testing inference...");
                    model.max_new_tokens = Some(10);
                    let inference_worked = model
                        .generate("Hello!")
                        .await
                        .map_err(|error| log::debug!("inference error: {error}"))
                        .is_ok();
                    log::debug!("inference_worked = {inference_worked:?}");

                    if inference_worked {
                        // yay!
                        model_id_state.set(ModelIdState::Valid);
                    }
                    else {
                        model_id_state.set(ModelIdState::Invalid {
                            reason: ModelIdInvalidReason::InferenceFailed,
                        });
                    }
                }
                else {
                    model_id_state.set(ModelIdState::Invalid {
                        reason: ModelIdInvalidReason::NotLoadable,
                    });
                }

                Ok::<(), Error>(())
            }
        }
    };

    let on_model_id_input = {
        let check_model = check_model.clone();

        move |event: Event| {
            let model_id = event_target_value(&event);

            if model_id.is_empty() {
                model_id_state.set(ModelIdState::default());
                model_search_results.set(vec![]);
                return;
            }

            let model_id = ModelId(model_id);

            let check_model = check_model.clone();

            spawn_local(
                async move {
                    let api = settings.with_untracked(|settings| settings.api());
                    let search_results = api.quick_search(&model_id.0, Some(5)).await?;

                    let exact_match = search_results
                        .models
                        .iter()
                        .find(|id| **id == model_id.0)
                        .is_some();
                    if exact_match {
                        check_model(model_id).await?;
                    }
                    else {
                        model_id_state.set(ModelIdState::Invalid {
                            reason: ModelIdInvalidReason::NotFound,
                        });
                    }

                    model_search_results.set(search_results.models);

                    Ok::<(), Error>(())
                }
                .map(move |result| {
                    if let Err(error) = result {
                        errors.push(error);
                    }
                }),
            );
        }
    };

    let on_model_id_selected = move |model_id: String| {
        let field = model_id_input_field.get_untracked().unwrap();
        field.set_value(&model_id);

        let model_id = ModelId(model_id);
        let check_model = check_model.clone();

        spawn_local(
            async move {
                check_model(model_id).await?;
                Ok::<(), Error>(())
            }
            .map(move |result| {
                if let Err(error) = result {
                    errors.push(error);
                }
            }),
        );
    };

    let on_model_id_input_debounced =
        use_debounce_fn_with_arg_and_options(on_model_id_input, 200.0, Default::default());

    let selected_model_data = create_memo(move |_| {
        // this intentionally only updates when the selected model changes, not the
        // settings.
        with!(|selected_model| {
            selected_model
                .get_model_id()
                .and_then(move |selected_model| {
                    settings.with_untracked(move |settings| {
                        settings.models.get(&selected_model).cloned()
                    })
                })
        })
    });

    let select_model = move |model| {
        match model {
            SelectedModel::Edit(_) => {
                model_name_invalid.set(false);
                model_id_state.set(ModelIdState::Valid);
            }
            SelectedModel::New => {
                model_name_invalid.set(true);
                model_id_state.set(ModelIdState::default());
            }
        }
        changes_saved.set(false);
        selected_model.set(model);
    };

    let delete_selected_model = move |_| {
        selected_model.try_update(|selected_model| {
            let selected_model = std::mem::replace(selected_model, SelectedModel::New);

            if let Some(model_id) = selected_model.get_model_id() {
                log::warn!("deleting model: {model_id}");

                update_settings.update(move |settings| {
                    settings.models.remove(model_id);
                });
            }
            else {
                log::warn!("delete modal confirmed without selected model");
            }
        });
    };

    let save_model = move || {
        let old_model_id = with!(|selected_model| selected_model.get_model_id().cloned());
        log::debug!("save model. old_model_id={old_model_id:?}");

        let name = model_name_input_field.get_untracked().unwrap().value();
        let new_model_id = ModelId(model_id_input_field.get_untracked().unwrap().value());
        let chat_template = model_chat_template_input_field
            .get_untracked()
            .unwrap()
            .value()
            .parse::<ChatTemplate>()
            .unwrap();
        let stream = model_stream_input_field.get_untracked().unwrap().checked();

        let model = Model {
            model_id: new_model_id.clone(),
            name: Some(name),
            chat_template,
            stream,
        };
        log::debug!("{model:#?}");

        update_settings.update(move |settings| {
            if let Some(old_model_id) = old_model_id {
                if old_model_id != new_model_id {
                    settings.models.remove(&old_model_id);
                }
            }
            settings.models.insert(new_model_id, model);
        });

        changes_saved.set(true);
    };

    view! {
        // delete model modal
        <div class="modal fade" id="settings_delete_model_modal" tabindex="-1">
            <div class="modal-dialog">
                <div class="modal-content">
                    <div class="modal-header">
                        <h5 class="modal-title">"Delete model"</h5>
                        <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
                    </div>
                    <div class="modal-body">
                        <p>
                            "This will delete the model "
                            <i>
                                {move || {
                                    with!(|selected_model_data| selected_model_data.as_ref().and_then(|model| model.name.clone()))
                                }}
                            </i>
                            "!"
                        </p>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">"Cancel"</button>
                        <button
                            type="button"
                            class="btn btn-danger"
                            data-bs-dismiss="modal"
                            on:click=delete_selected_model
                        >
                            "Delete"
                        </button>
                    </div>
                </div>
            </div>
        </div>

        <div class="d-flex flex-row w-100 flex-grow-1">
            // model list
            <div class="d-flex flex-column m-4 w-25 h-100 mh-100">
                <div class="pe-1 w-100">
                    <button
                        type="button"
                        class="btn btn-primary mb-2 w-100"
                        on:click=move |_| select_model(SelectedModel::New)
                    >
                        <span class="me-1"><BootstrapIcon icon="plus-circle-fill" /></span>
                        "New model"
                    </button>
                </div>
                <div class="flex-grow-1">
                    <div class="overflow-y-scroll h-75 pe-1 w-100">
                        <div class="list-group w-100">
                            <For
                                each=move || {
                                    let mut items = with!(|settings| settings.models.iter().map(|(id, model)| (id.clone(), model.display_name().to_lowercase())).collect::<Vec<_>>());
                                    items.sort_by_cached_key(|(_, display_name)| display_name.clone());
                                    items
                                }
                                key=|(id, _)| id.clone()
                                children=move |(id, _)| {
                                    let id2 = id.clone();
                                    view!{
                                        <button
                                            type="button"
                                            class="list-group-item list-group-item-action text-truncate"
                                            class:active=move || {
                                                let id = id2.clone();
                                                with!(|selected_model| selected_model.is_model(&id))
                                            }
                                            on:click={
                                                let id = id2.clone();
                                                move |_| select_model(SelectedModel::Edit(id.clone()))
                                            }
                                        >
                                            {move || {
                                                with!(|settings| settings.models.get(&id).map(|model| model.display_name().to_owned()))
                                            }}
                                        </button>
                                    }
                                }
                            />
                        </div>
                    </div>
                </div>
            </div>

            // edit/add model form
            <form class="d-flex flex-column mt-4 me-4 flex-grow-1 needs-validation" on:submit=|event| event.prevent_default() novalidate>
                <div class="d-flex flex-row mb-4">
                    <h4 class="m-0">
                        {move || with!(|selected_model| {
                            match selected_model {
                                SelectedModel::New => "New model",
                                SelectedModel::Edit(_) => "Edit model",
                            }
                        })}
                    </h4>
                    {move || changes_saved.get().then(|| {
                        view! {
                            <small class="text-success-emphasis ms-3 mt-auto">
                                <span class="me-1"><BootstrapIcon icon="floppy-fill" /></span>
                                "Saved"
                            </small>
                        }
                    })}
                </div>

                // name input
                <div class="form-floating mb-3">
                    <input
                        type="text"
                        class="form-control form-control-lg"
                        class:is-valid=move || !model_name_invalid.get()
                        class:is-invalid=model_name_invalid
                        id="model_name_input"
                        node_ref=model_name_input_field
                        prop:value=move || with!(|selected_model_data| {
                            selected_model_data.as_ref()
                                .and_then(|model| model.name.clone())
                                .unwrap_or_default()
                        })
                        on:input=move |event| {
                            let value = event_target_value(&event);
                            model_name_invalid.set(value.is_empty());
                            changes_saved.set(false);
                        }
                    />
                    <label
                        for="model_name_input"
                        class:text-danger-emphasis=model_name_invalid
                        class:text-success-emphasis=move || !model_name_invalid.get()
                    >
                    {move || {
                        if model_name_invalid.get() {
                            "Choose a name".into_view()
                        }
                        else {
                            "Name".into_view()
                        }
                    }}
                    </label>
                </div>

                // model id input
                <div class="dropdown mb-3">
                    <div class="form-floating" data-bs-toggle="dropdown">
                        <input
                            type="text"
                            class="form-control"
                            class:is-valid=move || model_id_state.get().is_valid()
                            class:is-invalid=move || model_id_state.get().is_invalid()
                            id="model_id_input"
                            node_ref=model_id_input_field
                            prop:value=move || with!(|selected_model| selected_model.get_model_id().map(|model_id| model_id.to_string()).unwrap_or_default())
                            on:input=move |event| {
                                on_model_id_input_debounced(event);
                                changes_saved.set(false);
                            }
                        />
                        <label
                            for="model_id_input"
                            class:text-danger-emphasis=move || model_id_state.get().is_invalid()
                            class:text-success-emphasis=move || model_id_state.get().is_valid()
                        >
                            {move || match model_id_state.get() {
                                ModelIdState::Checking => view!{
                                    "Checking"
                                    <div class="spinner-border spinner-border-sm ms-1" role="status"></div>
                                }.into_view(),
                                ModelIdState::Valid => "Model ID".into_view(),
                                ModelIdState::Invalid { reason } => reason.get_message().into_view()
                            }}
                        </label>
                    </div>
                    <ul
                        class="dropdown-menu w-100 shadow"
                        class:visually-hidden=move || with!(|model_search_results| model_search_results.is_empty())
                    >
                        <For
                            each=move || with!(|model_search_results| model_search_results.iter().cloned().collect::<Vec<_>>())
                            key=move |id| id.clone()
                            children=move |id| {
                                let id2 = id.clone();
                                view! {
                                    <li>
                                        <button
                                            type="button"
                                            href="#"
                                            class="dropdown-item"
                                            on:click={
                                                let on_model_id_selected = on_model_id_selected.clone();
                                                move |_| {
                                                    on_model_id_selected(id.clone());
                                                    changes_saved.set(false);
                                                }
                                            }
                                        >
                                            {id2}
                                        </button>
                                    </li>
                                }
                            }
                        />
                    </ul>
                    {move || {
                        model_id_state.get().is_valid_model_id().then(move || {
                            let field = model_id_input_field.get_untracked().unwrap();
                            let model_id = ModelId(field.value());
                            let url = model_id.url();
                            view!{
                                <div class="mt-1">
                                    <span class="me-1"><BootstrapIcon icon="info-circle" /></span>
                                    "Make sure to check out the "
                                    <a href={url} target="_blank">
                                        "model page"
                                        <BootstrapIcon icon="link-45deg" />
                                    </a>
                                </div>
                            }
                        })
                    }}
                </div>

                // chat template input
                <div class="form-floating mb-3">
                    <select
                        class="form-select"
                        id="model_chat_template_select"
                        node_ref=model_chat_template_input_field
                        aria-label="Select chat template"
                        on:input=move |_| changes_saved.set(false)
                    >
                        <For
                            each=move || { <ChatTemplate as VariantArray>::VARIANTS.into_iter() }
                            key=|chat_template| *chat_template
                            children=move |chat_template| view!{
                                <option
                                    value={chat_template.as_ref()}
                                    selected=move || {
                                        *chat_template == with!(|selected_model_data| {
                                            selected_model_data.as_ref()
                                                .map(|model| model.chat_template)
                                                .unwrap_or_default()
                                        })
                                    }
                                >
                                    {chat_template.get_message()}
                                </option>
                            }
                        />
                    </select>
                    <label for="model_chat_template_select">"Select a chat template"</label>
                </div>

                // stream toggle
                <div class="form-check form-switch mb-3">
                    <input
                        class="form-check-input"
                        type="checkbox"
                        role="switch"
                        id="model_stream_switch"
                        node_ref=model_stream_input_field
                        prop:checked=move || with!(|selected_model_data| {
                            selected_model_data.as_ref()
                                .map(|model| model.stream)
                                .unwrap_or(true)
                        })
                    />
                    <label class="form-check-label" for="model_stream_switch">"Stream assistant response (this is not supported by some models)"</label>
                </div>

                // buttons
                <div class="d-flex flex-row w-100 justify-content-end">
                    {move || with!(|selected_model| {
                        selected_model.is_edit().then(move || {
                            view!{
                                <button
                                    type="button"
                                    class="btn btn-danger w-25"
                                    data-bs-toggle="modal"
                                    data-bs-target="#settings_delete_model_modal"
                                >
                                    <span class="me-1"><BootstrapIcon icon="trash-fill" /></span>
                                    "Delete model"
                                </button>
                            }
                        })
                    })}
                    <button
                        type="button"
                        class="btn btn-primary w-25 ms-2"
                        disabled=move || !model_id_state.get().is_valid() || model_name_invalid.get()
                        on:click=move |_| save_model()
                    >
                        <span class="me-1"><BootstrapIcon icon="floppy-fill" /></span>
                        "Save changes"
                    </button>
                </div>
            </form>
        </div>
    }
}

#[component]
fn DebugTab() -> impl IntoView {
    let Context {
        errors,
        settings,
        update_settings,
        ..
    } = expect_context();

    let emit_error_input = create_node_ref::<Input>();

    view! {
        <div class="modal fade" id="settings_general_reset_modal" tabindex="-1">
            <div class="modal-dialog">
                <div class="modal-content">
                    <div class="modal-header">
                        <h5 class="modal-title">"Reset app"</h5>
                        <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
                    </div>
                    <div class="modal-body">
                        <p>"This will delete all conversations and settings for this app!"</p>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">"Cancel"</button>
                        <button
                            type="button"
                            class="btn btn-danger"
                            data-bs-dismiss="modal"
                            on:click=|_| {
                                log::warn!("clearing local storage");
                                clear_storage();
                            }
                        >
                            "Reset"
                        </button>
                    </div>
                </div>
            </div>
        </div>

        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100 w-75 mx-auto">
            <div class="form-check form-switch mb-2">
                <input
                    class="form-check-input"
                    type="checkbox"
                    role="switch"
                    checked=move || with!(|settings| settings.debug_mode)
                    on:input=move |event| update_settings.update(move |settings| settings.debug_mode = event_target_checked(&event))
                />
                <label class="form-check-label">"Debug mode"</label>
            </div>

            <div class="d-flex flex-row mb-3">
                <button
                    type="button"
                    class="btn btn-danger me-3"
                    data-bs-toggle="modal"
                    data-bs-target="#settings_general_reset_modal"
                >
                    <span class="me-2"><BootstrapIcon icon="exclamation-triangle-fill" /></span>
                    "Reset app"
                </button>
                <button
                    type="button"
                    class="btn btn-danger me-3"
                    on:click=move |_| {
                        update_settings.update(|settings| *settings = Default::default());
                    }
                >
                    <span class="me-2"><BootstrapIcon icon="exclamation-triangle-fill" /></span>
                    "Reset settings"
                </button>
                <button
                    type="button"
                    class="btn btn-danger me-3"
                    on:click=move |_| {
                        update_settings.update(|settings| settings.reset_models());
                    }
                >
                    <span class="me-2"><BootstrapIcon icon="exclamation-triangle-fill" /></span>
                    "Reset models"
                </button>
            </div>
            <form
                class="w-100"
                on:submit=move |e| {
                    e.prevent_default();

                    let message = emit_error_input.get_untracked().unwrap().value();
                    if !message.is_empty() {
                        errors.push(message);
                    }
                }
            >
                <div class="input-group mb-3">
                    <input type="text" class="form-control" placeholder="Error message" node_ref=emit_error_input />
                    <button
                        class="btn btn-primary"
                        type="submit"
                        on:click=move |_| {

                        }
                    >
                        "Emit error"
                    </button>
                </div>
            </form>
        </div>
    }
}
