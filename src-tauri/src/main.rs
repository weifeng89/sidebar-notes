// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "cli") {
        std::process::exit(stik_lib::run_cli(&args[1..]));
    }
    if args.first().is_some_and(|arg| arg == "remote") {
        std::process::exit(stik_lib::run_remote());
    }
    stik_lib::run();
}
