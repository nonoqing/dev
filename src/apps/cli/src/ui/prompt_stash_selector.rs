use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{layout::Rect, Frame};

use crate::{
    prompt_stash::PromptStashEntry,
    ui::{
        conversation_selector::{
            ConversationPoint, ConversationSelectorAction, ConversationSelectorState,
        },
        theme::Theme,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PromptStashAction {
    None,
    Select(String),
    Delete(String),
    Close,
}

pub(super) struct PromptStashSelectorState {
    selector: ConversationSelectorState,
}

impl PromptStashSelectorState {
    pub(super) fn new() -> Self {
        Self {
            selector: ConversationSelectorState::new("Stash", "Restore"),
        }
    }

    pub(super) fn show(&mut self, entries: Vec<PromptStashEntry>) {
        self.selector.set_pending_delete_id(None);
        self.selector.show(
            entries
                .into_iter()
                .map(|entry| {
                    let line_count = entry.draft.text.lines().count();
                    let footer = build_footer(entry.timestamp_ms, line_count);
                    let title = build_title(&entry.draft.text);
                    ConversationPoint::new(entry.id, title, footer)
                })
                .collect(),
        );
    }

    pub(super) fn hide(&mut self) {
        self.selector.set_pending_delete_id(None);
        self.selector.hide();
    }

    pub(super) fn reshow(&mut self) {
        self.selector.reshow();
    }

    pub(super) fn is_visible(&self) -> bool {
        self.selector.is_visible()
    }

    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> PromptStashAction {
        if key.kind != KeyEventKind::Press {
            return PromptStashAction::None;
        }
        // Intercept Ctrl+D for delete confirmation before delegating.
        if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.handle_delete();
        }
        match self.selector.handle_key_event(key) {
            ConversationSelectorAction::Select(id) => PromptStashAction::Select(id),
            ConversationSelectorAction::Close => PromptStashAction::Close,
            ConversationSelectorAction::Move(_) => {
                // Moving the selection resets delete confirmation.
                self.selector.set_pending_delete_id(None);
                PromptStashAction::None
            }
            ConversationSelectorAction::None => PromptStashAction::None,
        }
    }

    fn handle_delete(&mut self) -> PromptStashAction {
        let Some(selected_id) = self.selector.selected_id() else {
            return PromptStashAction::None;
        };
        match self.selector.pending_delete_id() {
            Some(pending) if pending == selected_id => {
                // Second press confirms deletion.
                self.selector.set_pending_delete_id(None);
                self.selector.hide();
                PromptStashAction::Delete(selected_id)
            }
            _ => {
                // First press arms confirmation for this entry.
                self.selector.set_pending_delete_id(Some(selected_id));
                PromptStashAction::None
            }
        }
    }

    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        self.selector.render(frame, area, theme);
    }
}

/// Build the list item title: the first line of the input, truncated to 50 chars.
fn build_title(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or("");
    let normalized = first_line.split_whitespace().collect::<Vec<_>>().join(" ");
    let max_chars = 50;
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut preview: String = normalized
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect();
    preview.push('\u{2026}');
    preview
}

/// Build the footer: relative time, optionally suffixed with line count.
fn build_footer(timestamp_ms: u64, line_count: usize) -> String {
    let time = relative_time(timestamp_ms);
    if line_count > 1 {
        format!("{time}  ~{line_count} lines")
    } else {
        time
    }
}

fn relative_time(timestamp_ms: u64) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let elapsed = now_ms.saturating_sub(timestamp_ms) / 1_000;
    const SEVEN_DAYS: u64 = 7 * 86_400;
    if elapsed >= SEVEN_DAYS {
        let secs = timestamp_ms / 1_000;
        if let Some(dt) = chrono::DateTime::from_timestamp(secs as i64, 0) {
            return dt
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string();
        }
    }
    match elapsed {
        0..=59 => "just now".to_string(),
        60..=3_599 => format!("{}m ago", elapsed / 60),
        3_600..=86_399 => format!("{}h ago", elapsed / 3_600),
        _ => format!("{}d ago", elapsed / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_stash_timestamps_fail_closed_to_just_now() {
        assert_eq!(relative_time(u64::MAX), "just now");
    }

    #[test]
    fn build_title_truncates_first_line_to_50_chars() {
        let short = "Hello world";
        assert_eq!(build_title(short), "Hello world");

        let multi_line = "First line\nSecond line";
        assert_eq!(build_title(multi_line), "First line");

        let long: String = "x".repeat(60);
        let title = build_title(&long);
        assert_eq!(title.chars().count(), 50);
        assert!(title.ends_with('\u{2026}'));
    }

    #[test]
    fn build_footer_shows_line_count_for_multiline() {
        let footer = build_footer(0, 3);
        assert!(footer.contains("~3 lines"));
    }

    #[test]
    fn build_footer_hides_line_count_for_single_line() {
        let footer = build_footer(0, 1);
        assert!(!footer.contains("lines"));
    }

    #[test]
    fn ctrl_d_first_press_arms_second_press_confirms() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut selector = PromptStashSelectorState::new();
        selector.show(vec![PromptStashEntry {
            id: "entry-1".to_string(),
            draft: crate::ui::composer::ComposerDraft::from_text("test prompt"),
            timestamp_ms: 0,
            workspace_identity: None,
        }]);

        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);

        // First press arms confirmation.
        assert_eq!(selector.handle_key_event(ctrl_d), PromptStashAction::None);
        assert!(selector.selector.pending_delete_id().is_some());

        // Second press confirms deletion.
        assert_eq!(
            selector.handle_key_event(ctrl_d),
            PromptStashAction::Delete("entry-1".to_string())
        );
        assert!(!selector.is_visible());
    }

    #[test]
    fn moving_selection_resets_delete_confirmation() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        let mut selector = PromptStashSelectorState::new();
        selector.show(vec![
            PromptStashEntry {
                id: "entry-1".to_string(),
                draft: crate::ui::composer::ComposerDraft::from_text("first"),
                timestamp_ms: 0,
                workspace_identity: None,
            },
            PromptStashEntry {
                id: "entry-2".to_string(),
                draft: crate::ui::composer::ComposerDraft::from_text("second"),
                timestamp_ms: 0,
                workspace_identity: None,
            },
        ]);

        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);

        // Arm confirmation on first entry.
        selector.handle_key_event(ctrl_d);
        assert!(selector.selector.pending_delete_id().is_some());

        // Move down resets confirmation.
        selector.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(selector.selector.pending_delete_id().is_none());
    }
}
