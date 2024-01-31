use leptos::{
    component,
    create_node_ref,
    html::Input,
    view,
    Children,
    IntoView,
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
    state::clear_storage,
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
            <Tab href="/settings/debug">"Debug"</Tab>
        </ul>
        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100 w-75 mx-auto">
            <Outlet />
        </div>
    }
}

#[component]
fn GeneralTab() -> impl IntoView {
    view! {
        "General tab"
    }
}

#[component]
fn BackendsTab() -> impl IntoView {
    view! {
        "Backends tab"
    }
}

#[component]
fn ModelsTab() -> impl IntoView {
    view! {
        "Models tab"
    }
}

#[component]
fn DebugTab() -> impl IntoView {
    let Context { errors, .. } = expect_context();

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

        <form on:submit=|e| e.prevent_default()>
            /*<div class="form-check form-switch mb-2">
                <input class="form-check-input" type="checkbox" role="switch" id="settings_general_dark_mode_switch" />
                <label class="form-check-label" for="settings_general_dark_mode_switch">"Use dark mode"</label>
            </div>*/
            <button
                type="button"
                class="btn btn-danger mb-3"
                data-bs-toggle="modal"
                data-bs-target="#settings_general_reset_modal"
            >
                <span class="me-2"><BootstrapIcon icon="exclamation-triangle-fill" /></span>
                "Reset app"
            </button>
            <div class="input-group mb-3">
                <input type="text" class="form-control" placeholder="Error message" node_ref=emit_error_input />
                <button
                    class="btn btn-primary"
                    type="button"
                    on:click=move |_| {
                        let message = emit_error_input.get_untracked().unwrap().value();
                        if !message.is_empty() {
                            errors.push(message);
                        }
                    }
                >
                    "Emit error"
                </button>
            </div>
        </form>
    }
}
