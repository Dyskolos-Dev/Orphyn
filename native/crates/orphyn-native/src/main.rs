// Release builds on Windows are GUI apps: no console window next to Orphyn.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! `cargo run -p orphyn-native` (see the crate docs for the development variables).

fn main() {
    orphyn_native::run();
}
