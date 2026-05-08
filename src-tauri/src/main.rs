// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // `daylog tui` and `daylog tui --help` route to the TUI binary entry
    // before any Tauri runtime spins up. Avoids the WebKit init cost on a
    // pure-terminal launch.
    if args.get(1).map(String::as_str) == Some("tui") {
        std::process::exit(daylog_tui::run(&args[2..]));
    }

    if args.iter().any(|a| a == "--uninstall-tracking") {
        eprintln!("Daylog: stopping tracker and removing background services…");
        match daylog_lib::uninstall_blocking() {
            Ok(()) => {
                eprintln!("Done. Your tracking data at ~/.local/share/activitywatch/ is preserved.");
                eprintln!("To remove the historical data as well, delete that directory manually.");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Uninstall reported an error: {e}");
                eprintln!("Some cleanup may be incomplete. Inspect:");
                eprintln!("  ~/.config/systemd/user/daylog-*.service");
                eprintln!("  ~/.config/autostart/daylog-tracker.desktop");
                eprintln!("  ~/.local/share/daylog/bin/");
                std::process::exit(1);
            }
        }
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("Daylog — local activity dashboard for ActivityWatch.");
        eprintln!();
        eprintln!("Usage:");
        eprintln!("  daylog                       Open the dashboard window.");
        eprintln!("  daylog tui                   Open the terminal dashboard.");
        eprintln!("  daylog --uninstall-tracking  Stop and remove the bundled tracker.");
        eprintln!("                               Leaves your data at ~/.local/share/activitywatch/ intact.");
        eprintln!("  daylog --help                Show this help.");
        std::process::exit(0);
    }

    daylog_lib::run()
}
