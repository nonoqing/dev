/// TUI interface module
///
/// Build terminal user interface using ratatui
pub(crate) mod agent_selector;
pub(crate) mod chat;
pub(crate) mod command_menu;
pub(crate) mod command_palette;
pub(crate) mod composer;
mod conversation_selector;
mod diff_render;
pub(crate) mod export_dialog;
pub(crate) mod fork_selector;
pub(crate) mod image_paste;
pub(crate) mod input;
pub(crate) mod login_form;
mod markdown;
pub(crate) mod mcp_add_dialog;
pub(crate) mod mcp_selector;
mod message_time;
pub(crate) mod model_config_form;
pub(crate) mod model_selector;
pub(crate) mod permission;
pub(crate) mod prompt_command_shell_review;
pub(crate) mod prompt_stash_selector;
pub(crate) mod provider_selector;
pub(crate) mod question;
mod responsive_popup;
mod selector_common;
pub(crate) mod session_lineage_selector;
pub(crate) mod session_selector;
pub(crate) mod skill_selector;
pub(crate) mod startup;
pub(crate) mod string_utils;
pub(crate) mod subagent_selector;
mod syntax_highlight;
mod text_input;
pub(crate) mod theme;
pub(crate) mod theme_selector;
pub(crate) mod timeline_selector;
mod tool_cards;
mod widgets;
pub(crate) mod workspace_diff;
pub(crate) mod workspace_reference;

use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Terminal,
};
use std::io;
use std::ops::{Deref, DerefMut};

type CliTerminal = Terminal<CrosstermBackend<io::Stdout>>;

pub(crate) struct TerminalGuard {
    terminal: Option<CliTerminal>,
}

impl Deref for TerminalGuard {
    type Target = CliTerminal;

    fn deref(&self) -> &Self::Target {
        self.terminal
            .as_ref()
            .expect("terminal guard must own a terminal")
    }
}

impl DerefMut for TerminalGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.terminal
            .as_mut()
            .expect("terminal guard must own a terminal")
    }
}

impl TerminalGuard {
    /// Temporarily return the process terminal to normal mode while a blocking
    /// foreground operation (such as an editor) inherits its stdio.
    pub(crate) fn with_restored<T>(&mut self, operation: impl FnOnce() -> T) -> Result<T> {
        let mut terminal = self
            .terminal
            .take()
            .expect("terminal guard must own a terminal");
        restore_terminal_inner(&mut terminal)?;
        drop(terminal);

        let operation_result = operation();

        let mut resumed = init_terminal()?;
        if let Err(error) = resumed.clear() {
            drop(resumed);
            return Err(error.into());
        }
        self.terminal = resumed.terminal.take();
        Ok(operation_result)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if let Some(mut terminal) = self.terminal.take() {
            let _ = restore_terminal_inner(&mut terminal);
        }
    }
}

/// Initialize terminal
pub(crate) fn init_terminal() -> Result<TerminalGuard> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    ) {
        let cleanup = cleanup_partial_terminal(&mut stdout);
        return Err(merge_terminal_failure(error, cleanup));
    }
    let backend = CrosstermBackend::new(stdout);
    let terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            let mut stdout = io::stdout();
            let cleanup = cleanup_partial_terminal(&mut stdout);
            return Err(merge_terminal_failure(error, cleanup));
        }
    };
    Ok(TerminalGuard {
        terminal: Some(terminal),
    })
}

/// Restore terminal
pub(crate) fn restore_terminal(mut guard: TerminalGuard) -> Result<()> {
    let result = guard
        .terminal
        .as_mut()
        .map(restore_terminal_inner)
        .unwrap_or(Ok(()));
    guard.terminal.take();
    result
}

fn restore_terminal_inner(terminal: &mut CliTerminal) -> Result<()> {
    let disable_raw = disable_raw_mode();
    let disable_bracketed_paste = execute!(terminal.backend_mut(), DisableBracketedPaste);
    let disable_mouse_capture = execute!(terminal.backend_mut(), DisableMouseCapture);
    let leave_alternate_screen = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let show_cursor = terminal.show_cursor();
    finish_terminal_cleanup([
        ("disable raw mode", disable_raw),
        ("disable bracketed paste", disable_bracketed_paste),
        ("disable mouse capture", disable_mouse_capture),
        ("leave alternate screen", leave_alternate_screen),
        ("show terminal cursor", show_cursor),
    ])
}

fn cleanup_partial_terminal(stdout: &mut io::Stdout) -> Result<()> {
    let disable_raw = disable_raw_mode();
    let disable_bracketed_paste = execute!(stdout, DisableBracketedPaste);
    let disable_mouse_capture = execute!(stdout, DisableMouseCapture);
    let leave_alternate_screen = execute!(stdout, LeaveAlternateScreen);
    finish_terminal_cleanup([
        ("disable raw mode", disable_raw),
        ("disable bracketed paste", disable_bracketed_paste),
        ("disable mouse capture", disable_mouse_capture),
        ("leave alternate screen", leave_alternate_screen),
    ])
}

fn finish_terminal_cleanup<const N: usize>(
    results: [(&'static str, std::io::Result<()>); N],
) -> Result<()> {
    let errors = results
        .into_iter()
        .filter_map(|(operation, result)| result.err().map(|error| format!("{operation}: {error}")))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(errors.join("; ")))
    }
}

fn merge_terminal_failure(primary: std::io::Error, cleanup: Result<()>) -> anyhow::Error {
    match cleanup {
        Ok(()) => primary.into(),
        Err(cleanup_error) => {
            let context = format!("{primary}; failed to restore the terminal: {cleanup_error}");
            anyhow::Error::new(primary).context(context)
        }
    }
}

// ── Terminal suspend/resume (Unix only) ──
// Implements Ctrl+Z suspend / `fg` resume via SIGTSTP/SIGCONT.
// On Windows these are not available; Ctrl+Z stays bound to undo.

#[cfg(unix)]
use ratatui::backend::Backend;

/// Suspend the TUI: restore terminal to original state, then send SIGTSTP
/// to the current process group. The OS suspends the process group;
/// execution blocks here until SIGCONT is received (via `fg`).
///
/// If SIGTSTP fails after the terminal has been restored, the TUI is
/// re-initialized so the caller does not end up in a broken half-restored
/// state.
#[cfg(unix)]
pub(crate) fn suspend_terminal(terminal: &mut Terminal<impl Backend>) -> Result<()> {
    // 1. Restore terminal to original state (leave alternate screen,
    //    disable raw mode, disable mouse capture, show cursor).
    //    Collect all errors rather than aborting on the first one so that
    //    we make as much progress as possible.
    let mut stdout = io::stdout();
    let mut errors = Vec::new();
    if let Err(error) = disable_raw_mode() {
        errors.push(format!("disable raw mode: {error}"));
    }
    if let Err(error) = execute!(
        stdout,
        DisableBracketedPaste,
        DisableMouseCapture,
        LeaveAlternateScreen
    ) {
        errors.push(format!("restore terminal screen: {error}"));
    }
    if let Err(error) = terminal.show_cursor() {
        errors.push(format!("show terminal cursor: {error}"));
    }
    if !errors.is_empty() {
        // Terminal restore failed; try to re-enter TUI mode to get back to
        // a known state before returning the error.
        let _ = resume_terminal(terminal);
        return Err(anyhow::anyhow!(errors.join("; ")));
    }
    // 2. Send SIGTSTP to the current process group (PID 0).
    //    The default OS action for SIGTSTP is to suspend the process.
    unsafe {
        if libc::kill(0, libc::SIGTSTP) != 0 {
            // SIGTSTP failed: the terminal is already restored to non-TUI
            // mode but the process was not suspended. Re-initialize the TUI
            // so the caller does not end up in a broken state.
            let _ = resume_terminal(terminal);
            return Err(anyhow::anyhow!(
                "failed to send SIGTSTP: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    Ok(())
}

/// Resume the TUI: re-enter alternate screen, re-enable raw mode,
/// re-enable mouse capture + bracketed paste, and force a full redraw.
///
/// All steps are attempted even if earlier ones fail, so the terminal
/// reaches the closest possible known state. Errors are aggregated.
#[cfg(unix)]
pub(crate) fn resume_terminal(terminal: &mut Terminal<impl Backend>) -> Result<()> {
    let mut errors = Vec::new();
    if let Err(error) = enable_raw_mode() {
        errors.push(format!("enable raw mode: {error}"));
    }
    let mut stdout = io::stdout();
    if let Err(error) = execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    ) {
        errors.push(format!("re-enter alternate screen: {error}"));
    }
    if let Err(error) = terminal.clear() {
        errors.push(format!("clear terminal: {error}"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(errors.join("; ")))
    }
}

/// Suspend the terminal and resume after `fg`.
///
/// Suspends via SIGTSTP (the OS blocks until SIGCONT is received via `fg`),
/// then re-initializes the terminal for TUI rendering. Callers in chat.rs
/// and startup.rs share this single path.
///
/// If suspend fails, `suspend_terminal` already attempted to re-enter TUI
/// mode, so the error is propagated without a redundant resume attempt.
#[cfg(unix)]
pub(crate) fn suspend_and_resume_terminal(terminal: &mut Terminal<impl Backend>) -> Result<()> {
    suspend_terminal(terminal).and_then(|_| resume_terminal(terminal))
}

/// Render a loading/status message on the terminal (stays in alternate screen)
pub(crate) fn render_loading(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    message: &str,
) -> Result<()> {
    let msg = message.to_string();
    terminal.draw(|frame| {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(45),
                Constraint::Length(3),
                Constraint::Percentage(45),
            ])
            .split(area);

        let text = vec![Line::from(Span::styled(
            msg,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))];

        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        frame.render_widget(paragraph, chunks[1]);
    })?;
    Ok(())
}

#[cfg(test)]
mod terminal_lifecycle_tests {
    use super::{finish_terminal_cleanup, merge_terminal_failure};

    #[test]
    fn terminal_cleanup_reports_every_failed_step_in_order() {
        let error = finish_terminal_cleanup([
            (
                "disable raw mode",
                Err(std::io::Error::other("raw failure")),
            ),
            (
                "disable bracketed paste",
                Err(std::io::Error::other("paste failure")),
            ),
            ("disable mouse capture", Ok(())),
            (
                "leave alternate screen",
                Err(std::io::Error::other("screen failure")),
            ),
            (
                "show terminal cursor",
                Err(std::io::Error::other("cursor failure")),
            ),
        ])
        .expect_err("cleanup failures must be reported")
        .to_string();

        assert_eq!(
            error,
            "disable raw mode: raw failure; disable bracketed paste: paste failure; leave alternate screen: screen failure; show terminal cursor: cursor failure"
        );
    }

    #[test]
    fn initialization_failure_without_cleanup_error_keeps_primary_io_error() {
        let error = merge_terminal_failure(
            std::io::Error::new(std::io::ErrorKind::NotConnected, "terminal unavailable"),
            Ok(()),
        );

        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::NotConnected)
        );
    }

    #[test]
    fn initialization_failure_keeps_primary_and_cleanup_diagnostics() {
        let cleanup = finish_terminal_cleanup([(
            "disable raw mode",
            Err(std::io::Error::other("cleanup failure")),
        )]);
        let error = merge_terminal_failure(
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "terminal initialization failure",
            ),
            cleanup,
        );
        let message = error.to_string();

        assert!(
            message.contains("terminal initialization failure"),
            "{message}"
        );
        assert!(
            message.contains("disable raw mode: cleanup failure"),
            "{message}"
        );
        assert_eq!(
            error
                .downcast_ref::<std::io::Error>()
                .map(std::io::Error::kind),
            Some(std::io::ErrorKind::PermissionDenied),
            "primary io::Error must remain in the anyhow source chain"
        );
    }
}
