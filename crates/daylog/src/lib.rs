//! Daylog — terminal screen time tracker for Linux. Ratatui surface
//! backed by `daylog-core`; first launch downloads the upstream tracker
//! (see `tracking/`).

use std::io;

mod app;
pub mod data;
pub mod theme;
pub mod tracking;
mod ui;
pub mod wizard;

pub use app::Tab;

enum Command {
    Dashboard,
    /// Force the wizard regardless of marker (`--setup`).
    Setup,
    /// Tear down systemd units / autostart entries / extracted binaries
    /// (`--uninstall-tracking`).
    UninstallTracking,
    Help,
    Version,
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        None => Ok(Command::Dashboard),
        Some("--setup") => Ok(Command::Setup),
        Some("--uninstall-tracking") => Ok(Command::UninstallTracking),
        Some("--help") | Some("-h") => Ok(Command::Help),
        Some("--version") | Some("-V") => Ok(Command::Version),
        Some(other) => Err(format!("unknown argument: {other}")),
    }
}

const HELP: &str = "\
daylog — terminal screen time and activity tracker for Linux

Usage:
  daylog                       Open the dashboard. On first launch, prompts
                               to install the bundled tracker.
  daylog --setup               Re-run the first-launch tracker installer.
  daylog --uninstall-tracking  Stop and remove the bundled tracker. Your
                               recorded data at ~/.local/share/activitywatch/
                               is preserved.
  daylog --help                Show this help.
  daylog --version             Print version and exit.
";

/// CLI entry point. Returns process exit code.
pub fn run(args: &[String]) -> i32 {
    let command = match parse_args(args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("daylog: {e}\n\n{HELP}");
            return 2;
        }
    };

    match command {
        Command::Help => {
            print!("{HELP}");
            return 0;
        }
        Command::Version => {
            println!("daylog {}", env!("CARGO_PKG_VERSION"));
            return 0;
        }
        Command::UninstallTracking => return run_uninstall(),
        Command::Setup | Command::Dashboard => {}
    }

    let force_wizard = matches!(command, Command::Setup);

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("daylog: failed to start tokio runtime: {e}");
            return 1;
        }
    };

    install_panic_handler();

    let exit = rt.block_on(async move { run_async(force_wizard).await });

    match exit {
        Ok(()) => 0,
        Err(e) => {
            // Terminal already restored by restore_terminal / panic handler.
            eprintln!("daylog: {e}");
            1
        }
    }
}

fn run_uninstall() -> i32 {
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("daylog: failed to start runtime: {e}");
            return 1;
        }
    };
    eprintln!("daylog: stopping tracker and removing background services…");
    match rt.block_on(tracking::uninstall()) {
        Ok(()) => {
            eprintln!("Done. Your data at ~/.local/share/activitywatch/ is preserved.");
            eprintln!("To remove the historical data as well, delete that directory manually.");
            0
        }
        Err(e) => {
            eprintln!("daylog: uninstall reported an error: {e}");
            eprintln!("Some cleanup may be incomplete. Inspect:");
            eprintln!("  ~/.config/systemd/user/daylog-*.service");
            eprintln!("  ~/.config/autostart/daylog-tracker.desktop");
            eprintln!("  ~/.local/share/daylog/bin/");
            1
        }
    }
}

async fn run_async(force_wizard: bool) -> io::Result<()> {
    let mut terminal = ui::setup_terminal()?;
    let mut app = app::App::new();

    let outcome = if force_wizard {
        wizard::run_forced(&mut terminal, &app.theme).await
    } else {
        wizard::run_if_needed(&mut terminal, &app.theme).await
    };

    match outcome {
        Ok(wizard::WizardOutcome::Quit) => {
            ui::restore_terminal(&mut terminal)?;
            return Ok(());
        }
        Ok(_) => {}
        Err(e) => {
            ui::restore_terminal(&mut terminal)?;
            eprintln!("daylog setup: {e}");
            return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
        }
    }

    terminal.draw(|f| ui::render(f, &app))?;

    let result = app::event_loop(&mut terminal, &mut app).await;
    ui::restore_terminal(&mut terminal)?;
    result
}

/// Restore the terminal on panic so users don't end up with a stuck raw
/// mode + alt screen + no echo.
fn install_panic_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ui::restore_terminal_raw();
        default_hook(info);
    }));
}
