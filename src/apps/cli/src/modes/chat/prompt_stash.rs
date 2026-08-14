use super::{ChatMode, ChatView};
use crate::prompt_stash;

impl ChatMode {
    /// Refresh the cached `stash_non_empty` flag from persistent storage.
    /// Called when the command palette opens and after stash mutations.
    pub(super) fn refresh_stash_non_empty(&mut self) {
        self.stash_non_empty = prompt_stash::is_stash_non_empty();
    }

    pub(super) fn stash_current_prompt(&mut self, chat_view: &mut ChatView) {
        let draft = chat_view.draft_snapshot();
        if chat_view.is_shell_mode() {
            chat_view.set_status(Some(
                "Prompt stash is unavailable in Shell mode; exit Shell mode first".to_string(),
            ));
            return;
        }
        if draft.text.trim().is_empty() {
            chat_view.set_status(Some("There is no prompt to stash".to_string()));
            return;
        }
        match prompt_stash::stash_prompt(&draft, self.workspace.as_deref()) {
            Ok(()) => {
                self.close_all_popups(chat_view);
                chat_view.clear_input();
                self.stash_non_empty = true;
                chat_view.set_status(Some("Prompt stashed".to_string()));
            }
            Err(error) => chat_view.set_status(Some(format!("Could not stash prompt: {error}"))),
        }
    }

    pub(super) fn pop_prompt_stash(&mut self, chat_view: &mut ChatView) {
        match prompt_stash::pop_stash(self.workspace.as_deref()) {
            Ok(Some((draft, references_detached))) => {
                self.close_all_popups(chat_view);
                chat_view.set_chat_draft(draft);
                self.refresh_stash_non_empty();
                chat_view.set_status(Some(prompt_stash::restored_status(
                    "Restored the latest stashed prompt",
                    references_detached,
                )));
            }
            Ok(None) => chat_view.set_status(Some("Prompt stash is empty".to_string())),
            Err(error) => {
                chat_view.set_status(Some(format!("Could not read prompt stash: {error}")))
            }
        }
    }

    pub(super) fn show_prompt_stash(&mut self, chat_view: &mut ChatView) {
        match prompt_stash::list_stash() {
            Ok(entries) if entries.is_empty() => {
                chat_view.set_status(Some("Prompt stash is empty".to_string()));
            }
            Ok(entries) => chat_view.show_prompt_stash_selector(entries),
            Err(error) => {
                chat_view.set_status(Some(format!("Could not read prompt stash: {error}")))
            }
        }
    }

    pub(super) fn restore_prompt_stash(&mut self, id: &str, chat_view: &mut ChatView) {
        match prompt_stash::restore_stash(id, self.workspace.as_deref()) {
            Ok(Some((draft, references_detached))) => {
                self.close_all_popups(chat_view);
                chat_view.set_chat_draft(draft);
                self.refresh_stash_non_empty();
                chat_view.set_status(Some(prompt_stash::restored_status(
                    "Restored stashed prompt",
                    references_detached,
                )));
            }
            Ok(None) => {
                self.close_all_popups(chat_view);
                chat_view.set_status(Some(
                    "That stashed prompt is no longer available; the list was refreshed elsewhere"
                        .to_string(),
                ));
            }
            Err(error) => chat_view.set_status(Some(format!("Could not restore prompt: {error}"))),
        }
    }

    pub(super) fn delete_prompt_stash(&mut self, id: &str, chat_view: &mut ChatView) {
        match prompt_stash::delete_stash_entry(id) {
            Ok(true) => {
                self.refresh_stash_non_empty();
                let entries = prompt_stash::list_stash().unwrap_or_default();
                if entries.is_empty() {
                    self.close_all_popups(chat_view);
                    chat_view.set_status(Some("Prompt stash is now empty".to_string()));
                } else {
                    chat_view.show_prompt_stash_selector(entries);
                    chat_view.set_status(Some("Stashed prompt deleted".to_string()));
                }
            }
            Ok(false) => {
                chat_view.set_status(Some(
                    "That stashed prompt is no longer available; the list was refreshed elsewhere"
                        .to_string(),
                ));
            }
            Err(error) => chat_view.set_status(Some(format!("Could not delete prompt: {error}"))),
        }
    }
}
