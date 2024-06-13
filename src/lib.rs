use cfg_if::cfg_if;
pub mod app;
pub mod auth;
#[cfg(feature = "ssr")]
pub mod aws;
pub mod blocks;
pub mod error_template;
#[cfg(feature = "ssr")]
pub mod fileserv;
#[cfg(feature = "ssr")]
pub(crate) mod github;
mod pages;
#[cfg(feature = "ssr")]
pub mod vercel_axum;
pub mod workflow;

cfg_if! { if #[cfg(feature = "hydrate")] {
    use leptos::*;
    use wasm_bindgen::prelude::wasm_bindgen;
    use crate::app::*;

    #[wasm_bindgen]
    pub fn hydrate() {
        _ = console_log::init_with_level(log::Level::Debug);
        console_error_panic_hook::set_once();

        leptos::mount_to_body(App);
    }
}}
