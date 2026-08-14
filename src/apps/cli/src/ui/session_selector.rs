/// Session selector popup for switching between sessions
///
/// Overlay popup that displays all available sessions
/// and allows the user to select one to switch to.
/// Supports switching and deleting current-project sessions.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::ui::selector_common::{backspace, delete_forward, insert_char, page_size_from_area};
use crate::ui::theme::{StyleKind, Theme};

// ── Constants ──

/// Color for group header text: rgb(53, 173, 48)
const GROUP_HEADER_COLOR: Color = Color::Rgb(53, 173, 48);
/// Title for the pinned-sessions group
const PINNED_GROUP_TITLE: &str = "Pinned";
/// Title for sessions modified today
const TODAY_GROUP_TITLE: &str = "Today";
/// Date format for non-today group titles (e.g. "Thu Jul 23 2026")
const DATE_GROUP_FORMAT: &str = "%a %b %e %Y";
/// Hint bar text shown in normal navigation mode
const HINT_NAVIGATE: &str =
    " Up/Down: Navigate  Enter: Switch  Ctrl+R: Rename  Ctrl+D: Delete  Ctrl+F: Pin  Esc: Close ";
/// Hint bar text shown when delete is not available
const HINT_NO_DELETE: &str =
    " Up/Down: Navigate  Enter: Switch  Ctrl+R: Rename  Ctrl+F: Pin  Esc: Close ";
/// Hint bar text shown in rename editing mode
const HINT_RENAME_EDIT: &str = " Enter: Confirm  Esc: Cancel ";

/// Maximum popup width (columns) for the session selector.
const MAX_POPUP_WIDTH: u16 = 70;
/// Padding subtracted from area width when computing popup width.
const POPUP_WIDTH_PADDING: u16 = 4;
/// Rows added to display row count for borders (top + bottom).
const POPUP_BORDER_ROWS: u16 = 2;
/// Rows added to display row count for the hint bar.
const POPUP_HINT_ROWS: u16 = 2;
/// Rows subtracted from area height to leave margin around the popup.
const POPUP_AREA_MARGIN: u16 = 2;
/// Minimum popup height (rows) below which the popup is not shown.
const MIN_POPUP_HEIGHT: u16 = 5;
/// Minimum popup width (columns) below which the popup is not shown.
const MIN_POPUP_WIDTH: u16 = 20;

/// A session item for display in the selector
#[derive(Debug, Clone)]
pub(crate) struct SessionItem {
    pub session_id: String,
    pub session_name: String,
    pub last_activity: String,
    pub workspace: Option<String>,
    /// Locally pinned (front-end only, not persisted to backend)
    pub pinned: bool,
    /// Original last-active-at timestamp in milliseconds (used for date grouping)
    pub last_active_at_ms: u64,
}

/// Actions emitted by the session selector back to the caller
#[derive(Debug, Clone)]
pub(crate) enum SessionAction {
    /// No action, selector consumed the event
    None,
    /// User selected a session to switch to
    Switch(SessionItem),
    /// User wants to delete the selected session
    Delete(SessionItem),
    /// User confirmed a rename operation with a new title
    Rename(SessionItem, String),
    /// User toggled pin status on a session (front-end only)
    PinToggle(SessionItem),
    /// User cancelled / closed the popup
    Close,
}

/// A display row in the grouped session list
#[derive(Debug, Clone)]
enum DisplayRow {
    /// A group header line (e.g. "Pinned" or "Today" or "Thu Jul 23 2026")
    Header(String),
    /// A spacer line (empty line between groups)
    Spacer,
    /// A session item row, referencing the original SessionItem by index
    Session(usize),
}

/// Session selector popup state
pub(super) struct SessionSelectorState {
    items: Vec<SessionItem>,
    /// Flattened display rows (headers, spacers, and session items)
    display_rows: Vec<DisplayRow>,
    list_state: ListState,
    visible: bool,
    /// Currently active session ID (for highlighting)
    current_session_id: Option<String>,
    can_delete: bool,
    last_area: Option<Rect>,
    /// When in rename editing mode, holds the buffer/cursor and the target session ID
    rename_mode: Option<RenameMode>,
}

/// Inline rename editing state within the session selector
struct RenameMode {
    session_id: String,
    buf: String,
    cursor: usize,
}

/// Compute the date group title for a timestamp in milliseconds.
/// Returns "Today" for today's date (local calendar date), or a formatted
/// date string like "Thu Jul 23 2026".
fn date_group_title(last_active_at_ms: u64) -> String {
    let now_local = chrono::Local::now();
    let item_timestamp = chrono::DateTime::from_timestamp_millis(last_active_at_ms as i64)
        .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH);
    let item_local: chrono::DateTime<chrono::Local> = chrono::DateTime::from(item_timestamp);

    if item_local.date_naive() == now_local.date_naive() {
        return TODAY_GROUP_TITLE.to_string();
    }

    item_local.format(DATE_GROUP_FORMAT).to_string()
}

/// Build the display rows from items, grouping pinned sessions under "Pinned"
/// and unpinned sessions by their last-activity date.
fn build_display_rows(items: &[SessionItem]) -> Vec<DisplayRow> {
    let mut rows: Vec<DisplayRow> = Vec::new();

    // Pinned group
    let pinned_items: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, s)| s.pinned)
        .map(|(i, _)| i)
        .collect();

    if !pinned_items.is_empty() {
        rows.push(DisplayRow::Header(PINNED_GROUP_TITLE.to_string()));
        for idx in &pinned_items {
            rows.push(DisplayRow::Session(*idx));
        }
    }

    // Date groups for unpinned items
    // Collect unpinned items sorted by last_active_at_ms descending (most recent first)
    let mut unpinned_items: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.pinned)
        .map(|(i, _)| i)
        .collect();
    unpinned_items.sort_by(|a, b| {
        items[*b]
            .last_active_at_ms
            .cmp(&items[*a].last_active_at_ms)
    });

    // Group by date
    let mut current_group_title: Option<String> = None;
    for idx in unpinned_items {
        let title = date_group_title(items[idx].last_active_at_ms);
        if current_group_title.as_ref() != Some(&title) {
            // Add spacer between groups (if there was a previous group)
            if current_group_title.is_some() || !pinned_items.is_empty() {
                rows.push(DisplayRow::Spacer);
            }
            rows.push(DisplayRow::Header(title.clone()));
            current_group_title = Some(title);
        }
        rows.push(DisplayRow::Session(idx));
    }

    rows
}

impl SessionSelectorState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            display_rows: Vec::new(),
            list_state: ListState::default(),
            visible: false,
            current_session_id: None,
            can_delete: false,
            last_area: None,
            rename_mode: None,
        }
    }

    /// Show the session selector with given session list
    pub(super) fn show(
        &mut self,
        sessions: Vec<SessionItem>,
        current_session_id: Option<String>,
        can_delete: bool,
    ) {
        if sessions.is_empty() {
            return;
        }

        self.items = sessions;
        self.display_rows = build_display_rows(&self.items);
        self.current_session_id = current_session_id.clone();

        // Select the first session row (skip headers/spacers)
        let initial_idx = current_session_id
            .as_ref()
            .and_then(|id| self.session_display_row_index(id))
            .unwrap_or_else(|| self.first_session_row_index());
        self.can_delete = can_delete;
        self.list_state.select(Some(initial_idx));
        self.visible = true;
    }

    pub(super) fn hide(&mut self) {
        self.visible = false;
        self.rename_mode = None;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the session selector (for back navigation)
    pub(super) fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether the selector is currently in rename editing mode
    pub(super) fn is_renaming(&self) -> bool {
        self.rename_mode.is_some()
    }

    /// Insert text into the rename buffer (for paste support).
    pub(super) fn insert_rename_text(&mut self, text: &str) {
        if let Some(ref mut rm) = self.rename_mode {
            for c in text.chars() {
                if c == '\n' || c == '\r' || c == '\t' {
                    continue;
                }
                insert_char(&mut rm.buf, &mut rm.cursor, c);
            }
        }
    }

    /// Remove item by session_id (after external deletion succeeds)
    pub(super) fn remove_item(&mut self, session_id: &str) {
        self.items.retain(|s| s.session_id != session_id);
        if self.items.is_empty() {
            self.hide();
            return;
        }
        // Rebuild display rows
        self.display_rows = build_display_rows(&self.items);
        // Clamp selection to a valid session row
        let selected = self.list_state.selected().unwrap_or(0);
        let new_selected = self.nearest_session_row_from(selected);
        self.list_state.select(Some(new_selected));
    }

    /// Update the display name of a session item (after rename).
    pub(super) fn update_item_name(&mut self, session_id: &str, new_name: &str) {
        if let Some(item) = self.items.iter_mut().find(|s| s.session_id == session_id) {
            item.session_name = new_name.to_string();
        }
    }

    /// Toggle pin status on a session and rebuild display rows.
    pub(super) fn toggle_pin(&mut self, session_id: &str) {
        if let Some(item) = self.items.iter_mut().find(|s| s.session_id == session_id) {
            item.pinned = !item.pinned;
        }
        // Rebuild display rows with updated grouping
        self.display_rows = build_display_rows(&self.items);
        // Re-select the toggled item
        if let Some(idx) = self.session_display_row_index(session_id) {
            self.list_state.select(Some(idx));
        }
    }

    /// Find the display row index of the session with given session_id.
    fn session_display_row_index(&self, session_id: &str) -> Option<usize> {
        // Find the item index first
        let item_idx = self.items.iter().position(|s| s.session_id == session_id)?;
        // Then find the display row that references this item
        self.display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Session(i) if *i == item_idx))
    }

    /// Find the first display row that is a session row.
    fn first_session_row_index(&self) -> usize {
        self.display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Session(_)))
            .unwrap_or(0)
    }

    /// Find the nearest session row from a given display row index,
    /// searching forward then backward.
    fn nearest_session_row_from(&self, from: usize) -> usize {
        // Search forward
        for i in from..self.display_rows.len() {
            if matches!(self.display_rows[i], DisplayRow::Session(_)) {
                return i;
            }
        }
        // Search backward
        for i in (0..from).rev() {
            if matches!(self.display_rows[i], DisplayRow::Session(_)) {
                return i;
            }
        }
        0
    }

    fn selected_item(&self) -> Option<&SessionItem> {
        let idx = self.list_state.selected()?;
        match self.display_rows.get(idx)? {
            DisplayRow::Session(item_idx) => self.items.get(*item_idx),
            _ => None,
        }
    }

    /// Handle a key event while the selector is visible.
    /// Returns a SessionAction describing what happened.
    pub(super) fn handle_key_event(&mut self, key: KeyEvent) -> SessionAction {
        if !self.visible {
            return SessionAction::None;
        }

        // ── Rename editing mode ──
        if self.rename_mode.is_some() {
            return self.handle_rename_key(key);
        }

        // ── Normal navigation mode ──
        match (key.code, key.modifiers) {
            (KeyCode::Up, _) => {
                self.move_up();
                SessionAction::None
            }
            (KeyCode::Down, _) => {
                self.move_down();
                SessionAction::None
            }
            (KeyCode::PageUp, _) => {
                self.move_page_up();
                SessionAction::None
            }
            (KeyCode::PageDown, _) => {
                self.move_page_down();
                SessionAction::None
            }
            (KeyCode::Enter, _) => {
                if let Some(item) = self.selected_item().cloned() {
                    self.hide();
                    SessionAction::Switch(item)
                } else {
                    SessionAction::None
                }
            }
            (KeyCode::Esc, _) => {
                self.hide();
                SessionAction::Close
            }
            // Ctrl+D: delete selected session
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                if !self.can_delete {
                    return SessionAction::None;
                }
                if let Some(item) = self.selected_item().cloned() {
                    SessionAction::Delete(item)
                } else {
                    SessionAction::None
                }
            }
            // Ctrl+R: enter rename editing mode for the selected session
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                if let Some(item) = self.selected_item().cloned() {
                    self.rename_mode = Some(RenameMode {
                        session_id: item.session_id.clone(),
                        buf: item.session_name.clone(),
                        cursor: item.session_name.chars().count(),
                    });
                }
                SessionAction::None
            }
            // Ctrl+F: toggle pin on selected session (front-end only)
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                if let Some(item) = self.selected_item().cloned() {
                    SessionAction::PinToggle(item)
                } else {
                    SessionAction::None
                }
            }
            _ => SessionAction::None,
        }
    }

    /// Handle keys while in rename editing mode.
    fn handle_rename_key(&mut self, key: KeyEvent) -> SessionAction {
        let rm = self.rename_mode.as_mut().unwrap();

        match key.code {
            KeyCode::Esc => {
                self.rename_mode = None;
                SessionAction::None
            }
            KeyCode::Enter => {
                let new_name = rm.buf.trim().to_string();
                let session_id = rm.session_id.clone();
                self.rename_mode = None;

                if let Some(item) = self
                    .items
                    .iter()
                    .find(|s| s.session_id == session_id)
                    .cloned()
                {
                    if new_name.is_empty() {
                        SessionAction::None
                    } else {
                        SessionAction::Rename(item, new_name)
                    }
                } else {
                    SessionAction::None
                }
            }
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+U: clear the rename buffer
                if c == 'u' || c == 'U' {
                    rm.buf.clear();
                    rm.cursor = 0;
                }
                SessionAction::None
            }
            KeyCode::Char(c) => {
                if c.is_control() || c == '\u{0}' {
                    SessionAction::None
                } else {
                    insert_char(&mut rm.buf, &mut rm.cursor, c);
                    SessionAction::None
                }
            }
            KeyCode::Backspace => {
                backspace(&mut rm.buf, &mut rm.cursor);
                SessionAction::None
            }
            KeyCode::Delete => {
                delete_forward(&mut rm.buf, &mut rm.cursor);
                SessionAction::None
            }
            KeyCode::Left => {
                rm.cursor = rm.cursor.saturating_sub(1);
                SessionAction::None
            }
            KeyCode::Right => {
                let max = rm.buf.chars().count();
                rm.cursor = (rm.cursor + 1).min(max);
                SessionAction::None
            }
            KeyCode::Home => {
                rm.cursor = 0;
                SessionAction::None
            }
            KeyCode::End => {
                rm.cursor = rm.buf.chars().count();
                SessionAction::None
            }
            // Allow navigation while in rename mode
            KeyCode::Up => {
                self.move_up();
                SessionAction::None
            }
            KeyCode::Down => {
                self.move_down();
                SessionAction::None
            }
            _ => SessionAction::None,
        }
    }

    /// Move selection to the previous session row (skip headers/spacers).
    fn move_up(&mut self) {
        let selected = self.list_state.selected().unwrap_or(0);
        // Search backward for a session row
        for i in (0..selected).rev() {
            if matches!(self.display_rows[i], DisplayRow::Session(_)) {
                self.list_state.select(Some(i));
                return;
            }
        }
        // Wrap: search from end backward
        for i in (selected..self.display_rows.len()).rev() {
            if matches!(self.display_rows[i], DisplayRow::Session(_)) {
                self.list_state.select(Some(i));
                return;
            }
        }
    }

    /// Move selection to the next session row (skip headers/spacers).
    fn move_down(&mut self) {
        let selected = self.list_state.selected().unwrap_or(0);
        // Search forward for a session row
        for i in (selected + 1)..self.display_rows.len() {
            if matches!(self.display_rows[i], DisplayRow::Session(_)) {
                self.list_state.select(Some(i));
                return;
            }
        }
        // Wrap: search from start forward
        for i in 0..selected {
            if matches!(self.display_rows[i], DisplayRow::Session(_)) {
                self.list_state.select(Some(i));
                return;
            }
        }
    }

    fn move_page_up(&mut self) {
        let page = page_size_from_area(self.last_area);
        let selected = self.list_state.selected().unwrap_or(0);
        // Try to jump `page` rows backward, landing on a session row
        let target = selected.saturating_sub(page);
        let idx = self.nearest_session_row_from(target);
        self.list_state.select(Some(idx));
    }

    fn move_page_down(&mut self) {
        let page = page_size_from_area(self.last_area);
        let selected = self.list_state.selected().unwrap_or(0);
        let target = (selected + page).min(self.display_rows.len().saturating_sub(1));
        let idx = self.nearest_session_row_from(target);
        self.list_state.select(Some(idx));
    }

    /// Render the session selector popup as an overlay
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.items.is_empty() {
            self.last_area = None;
            return;
        }

        let display_row_count = self.display_rows.len();
        let renaming = self.rename_mode.is_some();
        let rename_session_id = self.rename_mode.as_ref().map(|rm| rm.session_id.as_str());

        let popup_width = area
            .width
            .saturating_sub(POPUP_WIDTH_PADDING)
            .min(MAX_POPUP_WIDTH);
        let popup_height = (display_row_count as u16 + POPUP_BORDER_ROWS + POPUP_HINT_ROWS)
            .min(area.height.saturating_sub(POPUP_AREA_MARGIN));
        if popup_height < MIN_POPUP_HEIGHT || popup_width < MIN_POPUP_WIDTH {
            self.last_area = None;
            return;
        }

        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };
        self.last_area = Some(popup_area);

        let header_style = Style::default()
            .fg(GROUP_HEADER_COLOR)
            .add_modifier(Modifier::BOLD);
        let list_items: Vec<ListItem> = self
            .display_rows
            .iter()
            .enumerate()
            .map(|(_row_idx, row)| match row {
                DisplayRow::Header(title) => {
                    ListItem::new(Line::from(Span::styled(title, header_style)))
                }
                DisplayRow::Spacer => ListItem::new(Line::from("")),
                DisplayRow::Session(item_idx) => {
                    let session = &self.items[*item_idx];
                    let is_current = self
                        .current_session_id
                        .as_ref()
                        .is_some_and(|id| id == &session.session_id);

                    let marker = if is_current { "● " } else { "  " };
                    let marker_style = if is_current {
                        theme.style(StyleKind::Success)
                    } else {
                        theme.style(StyleKind::Muted)
                    };

                    let pin_marker = if session.pinned { "* " } else { "" };
                    let pin_style = Style::default().fg(Color::Yellow);

                    // If this is the row being renamed, render inline edit display
                    let is_renaming_row =
                        renaming && rename_session_id == Some(session.session_id.as_str());

                    if is_renaming_row {
                        let rm = self.rename_mode.as_ref().unwrap();
                        let name_style = Style::default().fg(Color::White);
                        let cursor_style = Style::default().fg(Color::Black).bg(Color::White);
                        let line = render_inline_edit_name(
                            marker,
                            marker_style,
                            pin_marker,
                            pin_style,
                            &rm.buf,
                            rm.cursor,
                            popup_width.saturating_sub(2) as usize,
                            name_style,
                            cursor_style,
                        );
                        ListItem::new(line)
                    } else {
                        let name_style =
                            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD);
                        let time_style = theme.style(StyleKind::Muted);
                        let workspace_style = Style::default().fg(Color::DarkGray);

                        let mut spans = vec![
                            Span::styled(marker, marker_style),
                            Span::styled(pin_marker, pin_style),
                            Span::styled(&session.session_name, name_style),
                        ];

                        // Show workspace path if available
                        if let Some(ref ws) = session.workspace {
                            let short_ws = std::path::Path::new(ws)
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| ws.clone());
                            spans.push(Span::styled(format!("  [{}]", short_ws), workspace_style));
                        }

                        spans.push(Span::styled(
                            format!("  {}", session.last_activity),
                            time_style,
                        ));

                        let line = Line::from(spans);
                        ListItem::new(line)
                    }
                }
            })
            .collect();

        let title = if renaming {
            " Rename Session "
        } else {
            " Switch Session "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(title);

        let list = List::new(list_items)
            .block(block)
            .style(Style::default().bg(theme.background))
            .highlight_style(
                Style::default()
                    .bg(theme.primary)
                    .fg(theme.selection_foreground())
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_widget(Clear, popup_area);
        frame.render_stateful_widget(list, popup_area, &mut self.list_state);

        // Position terminal cursor at the rename input position on the selected row
        if renaming {
            use unicode_width::UnicodeWidthChar;

            let rm = self.rename_mode.as_ref().unwrap();
            if let Some(selected) = self.list_state.selected() {
                // List content starts below the top border; account for scroll offset
                let offset = self.list_state.offset();
                let inner_y = popup_area.y + 1;
                let row_y = inner_y + (selected.saturating_sub(offset)) as u16;

                // Compute prefix display width (marker + pin_marker)
                let is_current = self
                    .current_session_id
                    .as_ref()
                    .is_some_and(|id| id == &rm.session_id);
                let marker = if is_current { "● " } else { "  " };
                let session = self.items.iter().find(|s| s.session_id == rm.session_id);
                let pinned = session.is_some_and(|s| s.pinned);
                let pin_marker = if pinned { "* " } else { "" };
                let prefix_w = unicode_width::UnicodeWidthStr::width(marker)
                    + unicode_width::UnicodeWidthStr::width(pin_marker);

                // Inner content width (inside borders) and name field width
                let inner_width = popup_width.saturating_sub(2) as usize;
                let name_field_width = inner_width.saturating_sub(prefix_w);

                // Cumulative display width up to cursor position
                let cursor_w: usize = rm
                    .buf
                    .chars()
                    .take(rm.cursor)
                    .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
                    .sum();

                // Scroll offset matching render_inline_edit_name (in display width)
                let scroll_w = if name_field_width == 0 || cursor_w < name_field_width / 3 {
                    0usize
                } else {
                    cursor_w.saturating_sub(name_field_width / 3)
                };

                let cursor_in_view_w = cursor_w.saturating_sub(scroll_w) as u16;

                let inner_x = popup_area.x + 1;
                frame.set_cursor_position((inner_x + prefix_w as u16 + cursor_in_view_w, row_y));
            }
        }

        // Render hint bar below the list
        let hint_y = popup_area.y + popup_area.height;
        if hint_y < area.y + area.height {
            let hint_area = Rect {
                x: popup_area.x,
                y: hint_y,
                width: popup_area.width,
                height: 1,
            };
            let hint_text = if renaming {
                HINT_RENAME_EDIT
            } else if self.can_delete {
                HINT_NAVIGATE
            } else {
                HINT_NO_DELETE
            };
            let hint = Paragraph::new(Line::from(Span::styled(
                hint_text,
                theme.style(StyleKind::Muted),
            )));
            frame.render_widget(hint, hint_area);
        }
    }

    /// Handle mouse events
    pub(super) fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> SessionAction {
        if !self.visible || self.rename_mode.is_some() {
            return SessionAction::None;
        }

        let area = match self.last_area {
            Some(area) => area,
            None => return SessionAction::None,
        };

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_popup => {
                self.move_up();
                SessionAction::None
            }
            MouseEventKind::ScrollDown if in_popup => {
                self.move_down();
                SessionAction::None
            }
            MouseEventKind::Moved if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    // Only select session rows, not headers/spacers
                    if matches!(self.display_rows.get(index), Some(DisplayRow::Session(_))) {
                        self.list_state.select(Some(index));
                    }
                }
                SessionAction::None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(index) = self.item_index_at(mouse.row, area) {
                    // Only act on session rows
                    if matches!(self.display_rows.get(index), Some(DisplayRow::Session(_))) {
                        self.list_state.select(Some(index));
                        if let Some(item) = self.selected_item().cloned() {
                            self.hide();
                            return SessionAction::Switch(item);
                        }
                    }
                }
                SessionAction::None
            }
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                SessionAction::Close
            }
            _ => SessionAction::None,
        }
    }

    pub(super) fn captures_mouse(&self, _mouse: &MouseEvent) -> bool {
        self.visible
    }

    fn item_index_at(&self, row: u16, area: Rect) -> Option<usize> {
        if area.height < 3 {
            return None;
        }
        let inner_y = area.y.saturating_add(1);
        let inner_height = area.height.saturating_sub(2);

        if row < inner_y || row >= inner_y.saturating_add(inner_height) {
            return None;
        }

        let offset = self.list_state.offset();
        let index = (row - inner_y) as usize + offset;
        if index >= self.display_rows.len() {
            return None;
        }

        Some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{backend::TestBackend, Terminal};

    fn make_items() -> Vec<SessionItem> {
        vec![SessionItem {
            session_id: "s1".to_string(),
            session_name: "My Session".to_string(),
            last_activity: "1m ago".to_string(),
            workspace: None,
            pinned: false,
            last_active_at_ms: 0,
        }]
    }

    fn make_state(items: Vec<SessionItem>) -> SessionSelectorState {
        let mut state = SessionSelectorState::new();
        state.show(items, None, true);
        state
    }

    #[test]
    fn ctrl_r_enters_rename_mode() {
        let mut state = make_state(make_items());
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        let action = state.handle_key_event(key);
        assert!(matches!(action, SessionAction::None));
        assert!(state.is_renaming());
    }

    #[test]
    fn typing_in_rename_mode_updates_buffer() {
        let mut state = make_state(make_items());
        // Enter rename mode
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        // Type 'X'
        state.handle_key_event(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE));
        // Verify buffer contains the character
        let rm = state.rename_mode.as_ref().unwrap();
        assert!(
            rm.buf.contains('X'),
            "buffer should contain 'X': got '{}'",
            rm.buf
        );
    }

    #[test]
    fn enter_confirms_rename() {
        let mut state = make_state(make_items());
        // Enter rename mode
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        // Clear and type new name
        state.handle_key_event(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
        state.handle_key_event(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE));
        state.handle_key_event(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE));
        state.handle_key_event(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        // Confirm with Enter
        let action = state.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        match action {
            SessionAction::Rename(item, name) => {
                assert_eq!(item.session_id, "s1");
                assert_eq!(name, "New");
            }
            _ => panic!("expected Rename action, got {:?}", action),
        }
    }

    #[test]
    fn esc_cancels_rename() {
        let mut state = make_state(make_items());
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        let action = state.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(action, SessionAction::None));
        assert!(!state.is_renaming());
    }

    #[test]
    fn render_shows_rename_input() {
        let mut state = make_state(make_items());
        // Enter rename mode
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        // Type a character
        state.handle_key_event(KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(40, 20)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render session selector in rename mode");

        let buffer = terminal.backend().buffer();
        let rendered: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // The buffer should contain 'Z' somewhere in the rename input line
        assert!(
            rendered.contains('Z'),
            "rendered output should contain 'Z':\n{rendered}"
        );
    }

    #[test]
    fn render_updates_after_backspace() {
        let mut state = make_state(make_items());
        // Enter rename mode (pre-filled with "My Session")
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
        // Backspace to delete last character
        state.handle_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));

        let mut terminal = Terminal::new(TestBackend::new(40, 20)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render session selector after backspace");

        let buffer = terminal.backend().buffer();
        let rendered: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // The buffer should show the truncated name (without the last char)
        assert!(
            rendered.contains("My Sessio"),
            "rendered output should contain truncated name:\n{rendered}"
        );
    }

    #[test]
    fn render_cursor_position_at_name_end() {
        let mut state = make_state(make_items());
        // Enter rename mode (pre-filled with "My Session", cursor at end = 10)
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));

        let mut terminal = Terminal::new(TestBackend::new(40, 20)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render session selector in rename mode");

        // With last_active_at_ms=0, build_display_rows produces:
        //   [Header("date"), Session(0)]  → display_row_count = 2
        // popup width = min(40-4, 70) = 36, popup_x = (40-36)/2 = 2
        // popup height = 2 + 2(border) + 2(hint) = 6, popup_y = (20-6)/2 = 7
        // selected = 1 (session row, after header), offset = 0
        // inner_x = 2 + 1 = 3, prefix = 2 (marker "  ", no pin), cursor_in_view = 10
        // expected_x = 3 + 2 + 10 = 15
        // row_y = 7 + 1 + (1 - 0) = 9
        let pos = terminal.get_cursor_position().expect("cursor position");
        assert_eq!(pos.x, 15, "cursor x should be 15");
        assert_eq!(pos.y, 9, "cursor y should be 9");
    }

    #[test]
    fn render_cursor_position_with_cjk_name() {
        // "我的会话" = 4 CJK chars, each 2 display columns wide = 8 total
        let items = vec![SessionItem {
            session_id: "s1".to_string(),
            session_name: "我的会话".to_string(),
            last_activity: "1m ago".to_string(),
            workspace: None,
            pinned: false,
            last_active_at_ms: 0,
        }];
        let mut state = make_state(items);
        state.handle_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));

        let mut terminal = Terminal::new(TestBackend::new(40, 20)).expect("test terminal");
        terminal
            .draw(|frame| state.render(frame, frame.area(), &Theme::dark_ansi16()))
            .expect("render session selector with CJK name");

        // popup width = min(40-4, 70) = 36, popup_x = (40-36)/2 = 2
        // popup height = 2 + 2 + 2 = 6, popup_y = (20-6)/2 = 7
        // selected = 1, offset = 0
        // inner_x = 3, prefix_w = 2 (marker "  ", no pin)
        // cursor at end of "我的会话" = char index 4, display width = 8
        // expected_x = 3 + 2 + 8 = 13
        // row_y = 7 + 1 + 1 = 9
        let pos = terminal.get_cursor_position().expect("cursor position");
        assert_eq!(
            pos.x, 13,
            "cursor x should be 13 (inner_x=3 + prefix=2 + cjk_width=8)"
        );
        assert_eq!(pos.y, 9, "cursor y should be 9");
    }
}

/// Render a session list row as an inline editable name field with a visible cursor.
/// `prefix_spans` provides the marker and pin marker; the editable buffer follows.
///
/// All width calculations use Unicode **display width** (via `unicode-width`)
/// so that CJK / wide characters occupying 2 terminal columns are handled correctly.
fn render_inline_edit_name<'a>(
    marker: &'a str,
    marker_style: Style,
    pin_marker: &'a str,
    pin_style: Style,
    buffer: &'a str,
    cursor: usize,
    available_width: usize,
    text_style: Style,
    cursor_style: Style,
) -> Line<'a> {
    use unicode_width::UnicodeWidthStr;

    let prefix_w = marker.width() + pin_marker.width();
    let field_width = available_width.saturating_sub(prefix_w);

    // Collect chars with their individual display widths.
    let chars: Vec<(char, usize)> = buffer
        .chars()
        .map(|c| {
            let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            (c, w)
        })
        .collect();

    // Cumulative display width up to each char index (exclusive).
    let cum_width: Vec<usize> = {
        let mut acc = 0usize;
        let mut v = Vec::with_capacity(chars.len() + 1);
        v.push(0);
        for &(_, w) in &chars {
            acc += w;
            v.push(acc);
        }
        v
    };

    let total_chars = chars.len();

    // Scroll offset (in char index) to keep cursor visible, reserving ~1/3
    // of field width on the left.
    let cursor_w = cum_width[cursor.min(total_chars)];

    let scroll = if field_width == 0 || cursor_w < field_width / 3 {
        0
    } else {
        // Find the char index whose cumulative width is the largest value
        // still <= (cursor_w - field_width/3)
        let target = cursor_w.saturating_sub(field_width / 3);
        // Binary search for the largest char index with cum_width <= target
        match cum_width.binary_search(&target) {
            Ok(idx) => idx,
            Err(idx) => idx, // idx is the first > target, so idx-1 is <= target; use idx-1 (clamp to 0)
        }
        .min(total_chars)
    };

    // Build visible substring from char index `scroll` up to field_width columns.
    let mut visible_chars: Vec<char> = Vec::new();
    let mut used_w = 0usize;
    for &(c, w) in &chars[scroll.min(total_chars)..] {
        if used_w + w > field_width && used_w > 0 {
            break;
        }
        used_w += w;
        visible_chars.push(c);
    }
    let cursor_in_view = cursor.saturating_sub(scroll).min(visible_chars.len());

    let before: String = visible_chars[..cursor_in_view].iter().collect();
    let cursor_char: String = visible_chars
        .get(cursor_in_view)
        .map(|c| c.to_string())
        .unwrap_or_else(|| " ".to_string());
    let after: String = if cursor_in_view + 1 <= visible_chars.len() {
        visible_chars[cursor_in_view + 1..].iter().collect()
    } else {
        String::new()
    };

    let cursor_display = if cursor_char.is_empty() {
        " ".to_string()
    } else {
        cursor_char
    };

    let has_more_left = scroll > 0;
    let has_more_right = scroll + visible_chars.len() < total_chars;

    let mut spans = vec![
        Span::styled(marker, marker_style),
        Span::styled(pin_marker, pin_style),
    ];

    if has_more_left {
        spans.push(Span::raw("\u{2190}"));
        // Drop one char to make room for the arrow
        let before_trimmed: String = before.chars().skip(1).collect();
        spans.push(Span::styled(before_trimmed, text_style));
    } else {
        spans.push(Span::styled(before, text_style));
    }

    spans.push(Span::styled(cursor_display, cursor_style));

    if has_more_right {
        let after_len = after.chars().count();
        if after_len > 0 {
            let after_trimmed: String = after.chars().take(after_len.saturating_sub(1)).collect();
            spans.push(Span::styled(after_trimmed, text_style));
        }
        spans.push(Span::raw("\u{2192}"));
    } else {
        spans.push(Span::styled(after, text_style));
    }

    Line::from(spans)
}
