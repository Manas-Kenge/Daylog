//! Daylog TUI — `daylog tui` entry point.
//!
//! Pure-Rust terminal surface mirroring the desktop dashboard's data
//! widgets. Shares the `daylog-core` data layer with the Tauri app, so
//! both surfaces see the same aw-server state and the same category
//! rules.
//!
//! Stage 1.A: skeleton (terminal setup, event loop, tab strip, footer,
//! help overlay). No data widgets yet — Today is empty.

use std::io;

mod app;
pub mod data;
pub mod theme;
mod ui;

pub use app::Tab;

/// CLI entry point invoked from `daylog tui`. Returns process exit code.
pub fn run(_args: &[String]) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("daylog tui: failed to start tokio runtime: {e}");
            return 1;
        }
    };

    install_panic_handler();

    let exit = rt.block_on(async move { run_async().await });

    match exit {
        Ok(()) => 0,
        Err(e) => {
            // Terminal already restored by the Drop guard / panic handler.
            eprintln!("daylog tui: {e}");
            1
        }
    }
}

async fn run_async() -> io::Result<()> {
    let mut terminal = ui::setup_terminal()?;
    // First frame: render the empty skeleton with loading state.
    let mut app = app::App::new();
    terminal.draw(|f| ui::render(f, &app))?;

    let result = app::event_loop(&mut terminal, &mut app).await;
    ui::restore_terminal(&mut terminal)?;
    result
}

/// Restore the terminal on panic so users don't end up with a stuck raw
/// mode + alt screen + no echo. Critical for the failure-mode review:
/// "panic handler restores terminal".
fn install_panic_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = ui::restore_terminal_raw();
        default_hook(info);
    }));
}
