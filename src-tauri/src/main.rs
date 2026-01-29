// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Entry point for the Tauri application.
/// Calls the `run` function from the library crate.
fn main() {
    tauri_appsona_lib::run()
}
