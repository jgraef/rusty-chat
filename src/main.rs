mod app;
mod state;
mod utils;

use wasm_bindgen::JsCast;

use crate::app::App;

fn main() {
    wasm_logger::init(wasm_logger::Config::new(log::Level::Trace));
    console_error_panic_hook::set_once();

    log::info!("starting app");

    let root = gloo_utils::document()
        .get_element_by_id("root")
        .expect("no root node found")
        .dyn_into()
        .unwrap();

    leptos::mount_to(root, App);
}
