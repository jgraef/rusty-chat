use leptos::{
    component,
    create_node_ref,
    event_target_checked,
    html::Input,
    view,
    with,
    Children,
    IntoView,
    SignalUpdate,
};
use leptos_router::{
    Outlet,
    Redirect,
    Route,
    ToHref,
    A,
};

use super::BootstrapIcon;
use crate::{
    app::{
        expect_context,
        Context,
    },
    state::{
        clear_storage,
        use_settings,
        StorageSignals,
    },
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
    let StorageSignals { read: settings, .. } = use_settings();

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
            <Tab href="/settings/backends">"Backends"</Tab>
            {move || {
                with!(|settings| settings.show_debug_tab)
                    .then(|| view!{
                        <Tab href="/settings/debug">"Debug"</Tab>
                    })
            }}
        </ul>
        <Outlet />
    }
}

#[component]
fn GeneralTab() -> impl IntoView {
    view! {
        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100 w-75 mx-auto">
            "General tab"
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
    view! {
        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100 w-75 mx-auto">
            "Models tab"
        </div>
    }
}

#[component]
fn DebugTab() -> impl IntoView {
    let Context { errors, .. } = expect_context();

    let StorageSignals {
        read: settings,
        write: update_settings,
        ..
    } = use_settings();

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
                    checked=move || with!(|settings| settings.show_debug_tab)
                    on:input=move |event| update_settings.update(move |settings| settings.show_debug_tab = event_target_checked(&event))
                />
                <label class="form-check-label">"Show debug tab"</label>
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
                        update_settings.update(|settings| settings.reset());
                    }
                >
                    <span class="me-2"><BootstrapIcon icon="exclamation-triangle-fill" /></span>
                    "Reset settings"
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
