/// Shared trait for popup-style selector states that use `ListState` and a
/// fixed-height fallback for page navigation.
///
/// Implementors only need to expose the last rendered area and the mutable
/// `ListState`. Default methods provide page-size computation and page
/// up/down navigation identical across all simple list selectors.
use ratatui::{layout::Rect, widgets::ListState};

/// Fallback height (rows) when the rendered area is unknown.
const FALLBACK_POPUP_HEIGHT: u16 = 10;
/// Rows subtracted from height to get visible content rows (border + padding).
const POPUP_HEIGHT_PADDING: u16 = 2;

/// Approximate visible rows from a popup area (clamped to >=1).
///
/// Shared by all selectors that use `last_area: Option<Rect>`, including
/// those with custom navigation logic (e.g. `ModelSelectorState` and
/// `SessionSelectorState`).
pub(super) fn page_size_from_area(last_area: Option<Rect>) -> usize {
    let h = last_area.map(|a| a.height).unwrap_or(FALLBACK_POPUP_HEIGHT);
    h.saturating_sub(POPUP_HEIGHT_PADDING) as usize
}

// ── Inline text-edit helpers ──
// Shared by session_selector (rename mode) and mcp_add_dialog to avoid
// duplicating simple char-cursor editing primitives.

/// Convert a char position to a byte position in the string.
pub(super) fn char_to_byte(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Insert a character at the cursor position.
pub(super) fn insert_char(buf: &mut String, cursor: &mut usize, c: char) {
    let byte_pos = char_to_byte(buf, *cursor);
    buf.insert(byte_pos, c);
    *cursor += 1;
}

/// Delete the character before the cursor (backspace).
pub(super) fn backspace(buf: &mut String, cursor: &mut usize) {
    if *cursor > 0 {
        *cursor -= 1;
        let byte_pos = char_to_byte(buf, *cursor);
        let next_byte = char_to_byte(buf, *cursor + 1);
        buf.replace_range(byte_pos..next_byte, "");
    }
}

/// Delete the character at the cursor position (forward delete).
pub(super) fn delete_forward(buf: &mut String, cursor: &mut usize) {
    let max = buf.chars().count();
    if *cursor < max {
        let byte_pos = char_to_byte(buf, *cursor);
        let next_byte = char_to_byte(buf, *cursor + 1);
        buf.replace_range(byte_pos..next_byte, "");
    }
}

/// Trait for selectors that use `ListState` and a `last_area: Option<Rect>`.
pub(super) trait PagedSelector {
    /// The last rendered popup area, if any.
    fn last_area(&self) -> Option<Rect>;

    /// Mutable access to the underlying `ListState`.
    fn list_state_mut(&mut self) -> &mut ListState;

    /// Number of selectable items.
    fn item_count(&self) -> usize;

    /// Whether the selector is currently visible.
    fn is_visible(&self) -> bool;

    /// Approximate visible rows from the last rendered area (clamped to >=1).
    fn page_size(&self) -> usize {
        page_size_from_area(self.last_area())
    }

    /// Move selection up by one page, clamping at the first item.
    fn move_page_up(&mut self) {
        if !self.is_visible() || self.item_count() == 0 {
            return;
        }
        let page = self.page_size();
        let selected = self.list_state_mut().selected().unwrap_or(0);
        let next = if selected >= page { selected - page } else { 0 };
        let next = next.min(self.item_count().saturating_sub(1));
        self.list_state_mut().select(Some(next));
    }

    /// Move selection down by one page, clamping at the last item.
    fn move_page_down(&mut self) {
        if !self.is_visible() || self.item_count() == 0 {
            return;
        }
        let page = self.page_size();
        let selected = self.list_state_mut().selected().unwrap_or(0);
        let next = (selected + page).min(self.item_count().saturating_sub(1));
        self.list_state_mut().select(Some(next));
    }
}
