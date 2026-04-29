// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    hostly_lib::elevation::relaunch_as_admin_if_needed();
    hostly_lib::run()
}
