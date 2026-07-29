mod api;
mod models;
mod views;

use dioxus::prelude::*;
use dioxus_logger::tracing::{info, Level};

use crate::views::Dashboard;

const STYLED_CSS: Asset = asset!("/assets/styled.css");

fn main() {
    dioxus_logger::init(Level::INFO).expect("failed to init logger");
    info!("starting my-reverse-proxy-ui");
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Stylesheet { href: STYLED_CSS }
        Dashboard {}
    }
}
