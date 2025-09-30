#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
use nimby_graph::App;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}
