/// Model selector popup for choosing AI model
///
/// Full-screen overlay popup that displays all available models
/// and allows the user to select one.
///
/// Favorited models are grouped under a "Favorites" header at the top of the
/// list; all remaining models appear under an "All Models" header below.
/// When there are no favorites, the "Favorites" header is omitted; when every
/// model is a favorite, the "All Models" header is omitted.
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::selector_common::page_size_from_area;
use crate::ui::theme::{StyleKind, Theme};

/// RGB color used for group header text: green (53, 173, 48).
const HEADER_COLOR: Color = Color::Rgb(53, 173, 48);

/// Group header labels.
const HEADER_FAVORITES: &str = "Favorites";
const HEADER_MODELS: &str = "Models";

/// Maximum popup width (columns) for the model selector.
const MAX_POPUP_WIDTH: u16 = 70;
/// Padding subtracted from the area width when computing popup width.
const POPUP_WIDTH_PADDING: u16 = 4;
/// Ratio of the area height used as the maximum popup height (0.75 = 75%).
const MAX_POPUP_HEIGHT_RATIO: f32 = 0.75;
/// Extra rows added to row count for border + title when computing ideal height.
const POPUP_HEIGHT_EXTRA_ROWS: u16 = 4;
/// Minimum popup height (rows) below which the popup is not shown.
const MIN_POPUP_HEIGHT: u16 = 8;
/// Minimum popup width (columns) below which the popup is not shown.
const MIN_POPUP_WIDTH: u16 = 20;
/// Rows subtracted from area height to leave margin around the popup.
const POPUP_AREA_MARGIN: u16 = 2;
/// Minimum inner height (rows) for rendering popup content.
const MIN_INNER_HEIGHT: u16 = 3;
/// Minimum inner width (columns) for rendering popup content.
const MIN_INNER_WIDTH: u16 = 4;
/// Inner padding (rows) subtracted from popup height for border + hint.
const POPUP_INNER_HEIGHT_PADDING: u16 = 2;
/// Inner padding (columns) subtracted from popup width for border.
const POPUP_INNER_WIDTH_PADDING: u16 = 2;

/// A model item for display in the selector
#[derive(Debug, Clone)]
pub(crate) struct ModelItem {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    /// Whether this model is marked as a favorite (persisted to backend config).
    pub favorite: bool,
}

/// Persist a model's favorite state to the backend config.
///
/// Spawns an async task that fetches the current model config from the
/// backend, updates its `favorite` field, and saves it back via
/// `ConfigService::update_ai_model`.
pub(crate) fn persist_model_favorite(model_id: &str, favorite: bool) {
    let model_id = model_id.to_string();
    tokio::spawn(async move {
        let config_service =
            match bitfun_core::service::config::GlobalConfigManager::get_service().await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to get config service for favorite persist: {}", e);
                    return;
                }
            };
        let models = match config_service.get_ai_models().await {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("Failed to get ai models for favorite persist: {}", e);
                return;
            }
        };
        if let Some(mut model) = models.into_iter().find(|m| m.id == model_id) {
            model.favorite = favorite;
            if let Err(e) = config_service.update_ai_model(&model_id, model).await {
                tracing::error!("Failed to persist favorite for model {}: {}", model_id, e);
            }
        } else {
            tracing::warn!("Model {} not found when persisting favorite", model_id);
        }
    });
}

// ── Flattened display row ──

#[derive(Debug, Clone)]
enum DisplayRow {
    /// Blank spacer line for vertical separation between groups
    Spacer,
    /// Group header ("Favorites" or "All Models")
    GroupHeader(String),
    /// A selectable model item (index into `items`)
    Model(usize),
}

/// Model selector popup state
pub(super) struct ModelSelectorState {
    items: Vec<ModelItem>,
    /// Flattened rows for rendering (headers + models)
    rows: Vec<DisplayRow>,
    /// Indices of selectable rows (into `rows`)
    selectable_row_indices: Vec<usize>,
    /// Currently highlighted selectable index (into `selectable_row_indices`)
    selected: usize,
    visible: bool,
    /// Currently active model ID (for highlighting)
    current_model_id: Option<String>,
    /// Viewport scroll offset
    scroll_offset: usize,
    /// Number of visible content rows (updated each frame)
    visible_rows: usize,
    /// Embedded startup may open model management; current-Session selection may not.
    allow_edit: bool,
    /// Selection changes the active Session rather than the future default.
    current_session_selection: bool,
    last_area: Option<Rect>,
}

impl ModelSelectorState {
    pub(super) fn new() -> Self {
        Self {
            items: Vec::new(),
            rows: Vec::new(),
            selectable_row_indices: Vec::new(),
            selected: 0,
            visible: false,
            current_model_id: None,
            allow_edit: false,
            current_session_selection: true,
            scroll_offset: 0,
            visible_rows: 0,
            last_area: None,
        }
    }

    /// Show the model selector with given model list
    pub(super) fn show(
        &mut self,
        models: Vec<ModelItem>,
        current_model_id: Option<String>,
        allow_edit: bool,
        current_session_selection: bool,
    ) {
        if models.is_empty() {
            return;
        }

        self.items = models;
        self.current_model_id = current_model_id;
        self.build_rows();

        // Initial selection: the current model if present, else first row.
        let initial = self
            .current_model_id
            .as_ref()
            .and_then(|id| self.find_selectable_index_by_model_id(id))
            .unwrap_or(0);
        self.selected = initial;
        self.scroll_offset = 0;
        self.allow_edit = allow_edit;
        self.current_session_selection = current_session_selection;
        self.visible = true;
    }

    /// Rebuild flattened display rows, grouping favorites under a
    /// "Favorites" header and the rest under a "Models" header.
    fn build_rows(&mut self) {
        self.rows.clear();
        self.selectable_row_indices.clear();

        let favorites: Vec<usize> = (0..self.items.len())
            .filter(|&i| self.items[i].favorite)
            .collect();
        let others: Vec<usize> = (0..self.items.len())
            .filter(|&i| !self.items[i].favorite)
            .collect();

        if !favorites.is_empty() {
            self.rows
                .push(DisplayRow::GroupHeader(HEADER_FAVORITES.into()));
            for i in favorites {
                self.selectable_row_indices.push(self.rows.len());
                self.rows.push(DisplayRow::Model(i));
            }
        }

        if !others.is_empty() {
            // Blank line separating groups
            if !self.rows.is_empty() {
                self.rows.push(DisplayRow::Spacer);
            }
            self.rows
                .push(DisplayRow::GroupHeader(HEADER_MODELS.into()));
            for i in others {
                self.selectable_row_indices.push(self.rows.len());
                self.rows.push(DisplayRow::Model(i));
            }
        }
    }

    /// Find the selectable index (into `selectable_row_indices`) for the given
    /// model id.
    fn find_selectable_index_by_model_id(&self, id: &str) -> Option<usize> {
        self.selectable_row_indices
            .iter()
            .position(|&row_idx| match self.rows.get(row_idx) {
                Some(DisplayRow::Model(i)) => self.items.get(*i).is_some_and(|m| m.id == id),
                _ => false,
            })
    }

    /// Hide the model selector
    pub(super) fn hide(&mut self) {
        self.visible = false;
        // Note: we don't clear items here to support back navigation
        self.last_area = None;
    }

    /// Reshow the model selector (for back navigation)
    pub(super) fn reshow(&mut self) {
        if !self.items.is_empty() {
            self.visible = true;
        }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }

    pub(super) fn allows_edit(&self) -> bool {
        self.allow_edit
    }

    pub(super) fn scope_hint(&self) -> &'static str {
        if self.current_session_selection {
            "Applies to the current session only"
        } else {
            "Default for future sessions"
        }
    }

    pub(super) fn move_up(&mut self) {
        if !self.visible || self.selectable_row_indices.is_empty() {
            return;
        }
        let len = self.selectable_row_indices.len();
        let next = (self.selected + len - 1) % len;
        self.selected = next;
        self.ensure_selection_visible();
    }

    pub(super) fn move_down(&mut self) {
        if !self.visible || self.selectable_row_indices.is_empty() {
            return;
        }
        let len = self.selectable_row_indices.len();
        let next = (self.selected + 1) % len;
        self.selected = next;
        self.ensure_selection_visible();
    }

    pub(super) fn move_page_up(&mut self) {
        if !self.visible || self.selectable_row_indices.is_empty() {
            return;
        }
        let page = page_size_from_area(self.last_area);
        let next = if self.selected >= page {
            self.selected - page
        } else {
            0
        };
        self.selected = next.min(self.selectable_row_indices.len().saturating_sub(1));
        self.ensure_selection_visible();
    }

    pub(super) fn move_page_down(&mut self) {
        if !self.visible || self.selectable_row_indices.is_empty() {
            return;
        }
        let page = page_size_from_area(self.last_area);
        let len = self.selectable_row_indices.len();
        let next = (self.selected + page).min(len.saturating_sub(1));
        self.selected = next;
        self.ensure_selection_visible();
    }

    /// Adjust `scroll_offset` so the currently selected row stays visible.
    fn ensure_selection_visible(&mut self) {
        if self.selectable_row_indices.is_empty() {
            return;
        }
        let selected_row_idx = self.selectable_row_indices[self.selected];
        let content_height = self.visible_rows.max(1);
        if self.rows.len() <= content_height {
            self.scroll_offset = 0;
            return;
        }
        let max_offset = self.rows.len() - content_height;
        if selected_row_idx < self.scroll_offset {
            self.scroll_offset = selected_row_idx;
        } else if selected_row_idx >= self.scroll_offset + content_height {
            self.scroll_offset = selected_row_idx + 1 - content_height;
        }
        self.scroll_offset = self.scroll_offset.min(max_offset);
    }

    /// Get the selected model item (returns clone of ModelItem)
    pub(super) fn confirm_selection(&self) -> Option<ModelItem> {
        if !self.visible {
            return None;
        }
        let row_idx = *self.selectable_row_indices.get(self.selected)?;
        match self.rows.get(row_idx)? {
            DisplayRow::Model(i) => self.items.get(*i).cloned(),
            _ => None,
        }
    }

    /// Toggle favorite status on the selected model and rebuild the grouped
    /// rows so the item moves between groups. The selection follows the
    /// toggled item so the cursor stays on it.
    pub(super) fn toggle_favorite(&mut self) {
        let Some(id) = self.confirm_selection().map(|m| m.id) else {
            return;
        };

        let mut new_favorite = None;
        if let Some(&row_idx) = self.selectable_row_indices.get(self.selected) {
            if let Some(DisplayRow::Model(i)) = self.rows.get(row_idx) {
                if let Some(item) = self.items.get_mut(*i) {
                    item.favorite = !item.favorite;
                    new_favorite = Some(item.favorite);
                }
            }
        }

        if let Some(favorite) = new_favorite {
            persist_model_favorite(&id, favorite);
        }

        self.build_rows();
        self.selected = self
            .find_selectable_index_by_model_id(&id)
            .unwrap_or(0)
            .min(self.selectable_row_indices.len().saturating_sub(1));
        self.ensure_selection_visible();
    }

    /// Render the model selector popup as an overlay
    pub(super) fn render(&mut self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if !self.visible || self.items.is_empty() {
            self.last_area = None;
            return;
        }

        let popup_area = match self.popup_rect(area) {
            Some(rect) => {
                self.last_area = Some(rect);
                rect
            }
            None => {
                self.last_area = None;
                return;
            }
        };

        self.render_border(frame, popup_area, theme);
        self.render_content(frame, popup_area, area, theme);
    }

    /// Compute the centered popup rectangle within the available area.
    fn popup_rect(&self, area: Rect) -> Option<Rect> {
        let popup_width = area
            .width
            .saturating_sub(POPUP_WIDTH_PADDING)
            .min(MAX_POPUP_WIDTH);
        let max_popup_height = (area.height as f32 * MAX_POPUP_HEIGHT_RATIO) as u16;
        let ideal_height = (self.rows.len() as u16 + POPUP_HEIGHT_EXTRA_ROWS).min(max_popup_height);
        let popup_height = ideal_height
            .max(MIN_POPUP_HEIGHT)
            .min(area.height.saturating_sub(POPUP_AREA_MARGIN));
        if popup_height < MIN_POPUP_HEIGHT || popup_width < MIN_POPUP_WIDTH {
            return None;
        }

        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;

        Some(Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        })
    }

    /// Render the popup border and title.
    fn render_border(&self, frame: &mut Frame, popup_area: Rect, theme: &Theme) {
        let title = if self.allow_edit {
            " Select Model (↑↓ Navigate, Enter Select, e Edit, Ctrl+A Provider, Ctrl+F Favorite, Esc Cancel) "
        } else {
            " Select Model (↑↓ Navigate, Enter Select, Esc Cancel) "
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(theme.style(StyleKind::Primary))
            .style(Style::default().bg(theme.background))
            .title(title);

        frame.render_widget(Clear, popup_area);
        frame.render_widget(block, popup_area);
    }

    /// Render the list rows and bottom hint inside the popup.
    fn render_content(&mut self, frame: &mut Frame, popup_area: Rect, area: Rect, theme: &Theme) {
        let inner = Rect {
            x: popup_area.x + 1,
            y: popup_area.y + 1,
            width: popup_area.width.saturating_sub(POPUP_INNER_WIDTH_PADDING),
            height: popup_area.height.saturating_sub(POPUP_INNER_HEIGHT_PADDING),
        };

        if inner.height < MIN_INNER_HEIGHT || inner.width < MIN_INNER_WIDTH {
            return;
        }

        // Content area (reserve 1 row for hint at bottom)
        let content_height = inner.height.saturating_sub(1) as usize;
        self.visible_rows = content_height;

        // Clamp scroll
        if self.rows.len() <= content_height {
            self.scroll_offset = 0;
        } else {
            let max_offset = self.rows.len() - content_height;
            self.scroll_offset = self.scroll_offset.min(max_offset);
        }

        let visible_end = (self.scroll_offset + content_height).min(self.rows.len());
        for (vi, row_idx) in (self.scroll_offset..visible_end).enumerate() {
            let row_y = inner.y + vi as u16;
            if row_y >= inner.y + inner.height.saturating_sub(1) {
                break;
            }
            let row_area = Rect {
                x: inner.x,
                y: row_y,
                width: inner.width,
                height: 1,
            };
            self.render_row(frame, row_area, row_idx, theme);
        }

        self.render_hint(frame, popup_area, area, theme);
    }

    /// Render a single display row (spacer, header, or model).
    fn render_row(&self, frame: &mut Frame, row_area: Rect, row_idx: usize, theme: &Theme) {
        let row = &self.rows[row_idx];
        match row {
            DisplayRow::Spacer => {
                frame.render_widget(Paragraph::new(Line::raw("")), row_area);
            }
            DisplayRow::GroupHeader(name) => {
                let header_line = Line::from(vec![Span::styled(
                    format!("  {}", name),
                    Style::default()
                        .fg(HEADER_COLOR)
                        .add_modifier(Modifier::BOLD),
                )]);
                frame.render_widget(Paragraph::new(header_line), row_area);
            }
            DisplayRow::Model(i) => {
                let model = &self.items[*i];
                let is_selected = self
                    .selectable_row_indices
                    .get(self.selected)
                    .is_some_and(|&ri| ri == row_idx);
                self.render_model_row(frame, row_area, model, is_selected, theme);
            }
        }
    }

    /// Render the bottom hint line below the popup.
    fn render_hint(&self, frame: &mut Frame, popup_area: Rect, area: Rect, theme: &Theme) {
        let hint_area = Rect {
            x: popup_area.x,
            y: popup_area.y + popup_area.height,
            width: popup_area.width,
            height: 1.min(area.y + area.height - popup_area.y - popup_area.height),
        };
        if hint_area.height > 0 {
            let hint = Paragraph::new(Line::from(vec![Span::styled(
                format!(" {} ", self.scope_hint()),
                theme.style(StyleKind::Info),
            )]))
            .alignment(Alignment::Center);
            frame.render_widget(hint, hint_area);
        }
    }

    fn render_model_row(
        &self,
        frame: &mut Frame,
        row_area: Rect,
        model: &ModelItem,
        is_selected: bool,
        theme: &Theme,
    ) {
        if is_selected {
            self.render_selected_bg(frame, row_area, theme);
        }

        let is_current = self
            .current_model_id
            .as_ref()
            .is_some_and(|id| id == &model.id);

        let marker = if is_current { "\u{25cf} " } else { "  " };
        let marker_style = if is_current {
            theme.style(StyleKind::Success)
        } else {
            theme.style(StyleKind::Muted)
        };

        let name_style = self.name_style(is_selected, theme);
        let detail_style = self.detail_style(is_selected, theme);
        let bg_style = self.row_bg_style(is_selected, theme);

        let line = Line::from(vec![
            Span::styled(marker, marker_style),
            Span::styled(&model.name, name_style),
            Span::styled("  ", bg_style),
            Span::styled(
                format!("[{}/{}]", model.provider, model.model_name),
                detail_style,
            ),
        ]);
        frame.render_widget(Paragraph::new(line), row_area);
    }

    /// Fill the row background when selected.
    fn render_selected_bg(&self, frame: &mut Frame, row_area: Rect, theme: &Theme) {
        let bg_fill = " ".repeat(row_area.width as usize);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                bg_fill,
                Style::default().bg(theme.primary),
            ))),
            row_area,
        );
    }

    /// Compute the name style for a model row based on selection state.
    fn name_style(&self, is_selected: bool, theme: &Theme) -> Style {
        if is_selected {
            Style::default()
                .bg(theme.primary)
                .fg(theme.selection_foreground())
                .add_modifier(Modifier::BOLD)
        } else {
            theme.style(StyleKind::Primary).add_modifier(Modifier::BOLD)
        }
    }

    /// Compute the detail style for a model row based on selection state.
    fn detail_style(&self, is_selected: bool, theme: &Theme) -> Style {
        if is_selected {
            Style::default()
                .bg(theme.primary)
                .fg(theme.selection_foreground())
        } else {
            theme.style(StyleKind::Muted)
        }
    }

    /// Compute the background style for a model row based on selection state.
    fn row_bg_style(&self, is_selected: bool, theme: &Theme) -> Style {
        if is_selected {
            Style::default().bg(theme.primary)
        } else {
            Style::default()
        }
    }

    /// Handle mouse events in the model selector
    /// Returns Some(model_id) if a model was clicked/selected, None otherwise
    pub(super) fn handle_mouse_event(&mut self, mouse: &MouseEvent) -> Option<ModelItem> {
        if !self.visible {
            return None;
        }

        let area = self.last_area?;

        let in_popup = mouse.column >= area.x
            && mouse.column < area.x.saturating_add(area.width)
            && mouse.row >= area.y
            && mouse.row < area.y.saturating_add(area.height);

        match mouse.kind {
            MouseEventKind::ScrollUp if in_popup => {
                self.move_up();
                None
            }
            MouseEventKind::ScrollDown if in_popup => {
                self.move_down();
                None
            }
            MouseEventKind::Moved if in_popup => {
                if let Some(index) = self.selectable_index_at(mouse.row, area) {
                    self.selected = index;
                }
                None
            }
            MouseEventKind::Down(MouseButton::Left) if in_popup => {
                if let Some(index) = self.selectable_index_at(mouse.row, area) {
                    self.selected = index;
                    return self.confirm_selection();
                }
                None
            }
            // Click outside popup to dismiss
            MouseEventKind::Down(MouseButton::Left) if !in_popup => {
                self.hide();
                None
            }
            _ => None,
        }
    }

    /// Check if a mouse event is within the popup area (used to prevent event passthrough)
    pub(super) fn captures_mouse(&self, _mouse: &MouseEvent) -> bool {
        if !self.visible {
            return false;
        }
        // When visible, capture all mouse events
        true
    }

    /// Map a mouse row to a selectable index (into `selectable_row_indices`).
    fn selectable_index_at(&self, row: u16, area: Rect) -> Option<usize> {
        if area.height < 3 {
            return None;
        }
        let inner_y = area.y.saturating_add(1); // border
        let inner_height = area.height.saturating_sub(2);

        if row < inner_y || row >= inner_y.saturating_add(inner_height) {
            return None;
        }

        let row_offset = self.scroll_offset;
        let relative_row = (row - inner_y) as usize;
        let row_idx = row_offset + relative_row;

        // Find the selectable_row_indices entry whose row index matches row_idx.
        self.selectable_row_indices
            .iter()
            .position(|&ri| ri == row_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelItem, ModelSelectorState};

    fn model() -> ModelItem {
        ModelItem {
            id: "model-1".to_string(),
            name: "Provider".to_string(),
            provider: "provider".to_string(),
            model_name: "Model 1".to_string(),
            favorite: false,
        }
    }

    #[test]
    fn current_session_picker_does_not_advertise_model_management() {
        let mut state = ModelSelectorState::new();
        state.show(vec![model()], Some("model-1".to_string()), false, true);

        assert!(!state.allows_edit());
        assert_eq!(state.scope_hint(), "Applies to the current session only");
    }

    #[test]
    fn startup_picker_keeps_embedded_model_management_and_default_scope() {
        let mut state = ModelSelectorState::new();
        state.show(vec![model()], Some("model-1".to_string()), true, false);

        assert!(state.allows_edit());
        assert_eq!(state.scope_hint(), "Default for future sessions");
    }

    #[test]
    fn embedded_chat_picker_can_edit_without_changing_the_selection_scope() {
        let mut state = ModelSelectorState::new();
        state.show(vec![model()], Some("model-1".to_string()), true, true);

        assert!(state.allows_edit());
        assert_eq!(state.scope_hint(), "Applies to the current session only");
    }
}
