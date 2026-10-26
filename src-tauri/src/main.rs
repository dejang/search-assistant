// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;

fn main() {
    let page_budget = Duration::from_secs(15);
    let page_budget_ms = page_budget.as_millis().to_string();
    unsafe { std::env::set_var("OBSCURA_SCRIPT_DEADLINE_MS", &page_budget_ms) };
    if let Err(error) = research_buddy_lib::run(page_budget) {
        eprintln!("research-buddy failed to start: {error}");
        std::process::exit(1);
    }
}
