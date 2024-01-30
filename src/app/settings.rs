use leptos::{
    component,
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
use crate::state::clear_storage;

#[component(transparent)]
pub fn SettingsRoutes() -> impl IntoView {
    view! {
        <Route path="/settings" view=Settings>
            <Route path="general" view=GeneralTab />
            <Route path="backends" view=BackendsTab />
            <Route path="models" view=ModelsTab />
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
        <div class="d-flex flex-row px-4 pt-2 w-100">
            <h4>
                <span class="me-2"><BootstrapIcon icon="gear-fill" /></span>
                Settings
            </h4>
        </div>
        <ul class="nav nav-tabs px-4 mt-2">
            <Tab href="/settings/general">"General"</Tab>
            <Tab href="/settings/backends">"Backends"</Tab>
            <Tab href="/settings/models">"Models"</Tab>
        </ul>
        <div class="d-flex flex-column overflow-y-scroll mb-auto p-4 mw-100">
            <Outlet />
        </div>
    }
}

#[component]
fn GeneralTab() -> impl IntoView {
    view! {
        <form on:submit=|e| e.prevent_default()>
            <button type="button" class="btn btn-danger" on:click=|_| clear_storage()>"Reset"</button>
        </form>
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
