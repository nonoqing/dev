use super::{ChatMode, ChatView};

impl ChatMode {
    /// Refresh the cached `stash_non_empty` flag from persistent storage.
    /// Called when the command palette opens and after stash mutations.
    pub(super) fn refresh_stash_non_empty(&mut self) {
        self.stash_non_empty = crate::prompt_stash::PromptStashStore::from_config_dir()
            .ok()
            .is_some_and(|store| store.is_non_empty().unwrap_or(false));
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
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        let result = crate::prompt_stash::PromptStashStore::from_config_dir()
            .map_err(|error| error.to_string())
            .and_then(|store| {
                store
                    .push(&draft, self.workspace.as_deref(), timestamp_ms)
                    .map_err(|error| error.to_string())
            });
        match result {
            Ok(_) => {
                self.close_all_popups(chat_view);
                chat_view.clear_input();
                self.stash_non_empty = true;
                chat_view.set_status(Some("Prompt stashed".to_string()));
            }
            Err(error) => chat_view.set_status(Some(format!("Could not stash prompt: {error}"))),
        }
    }

    pub(super) fn pop_prompt_stash(&mut self, chat_view: &mut ChatView) {
        let result = crate::prompt_stash::PromptStashStore::from_config_dir()
            .map_err(|error| error.to_string())
            .and_then(|store| store.pop().map_err(|error| error.to_string()));
        match result {
            Ok(Some(entry)) => {
                let (draft, references_detached) =
                    entry.into_draft_for_workspace(self.workspace.as_deref());
                self.close_all_popups(chat_view);
                chat_view.set_chat_draft(draft);
                self.refresh_stash_non_empty();
                chat_view.set_status(Some(restored_status(
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
        let result = crate::prompt_stash::PromptStashStore::from_config_dir()
            .map_err(|error| error.to_string())
            .and_then(|store| store.list().map_err(|error| error.to_string()));
        match result {
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
        let result = crate::prompt_stash::PromptStashStore::from_config_dir()
            .map_err(|error| error.to_string())
            .and_then(|store| {
                let entry = store
                    .list()
                    .map_err(|error| error.to_string())?
                    .into_iter()
                    .find(|entry| entry.id == id);
                let Some(entry) = entry else {
                    return Ok(None);
                };
                if !store.remove(id).map_err(|error| error.to_string())? {
                    return Ok(None);
                }
                Ok(Some(entry))
            });
        match result {
            Ok(Some(entry)) => {
                self.close_all_popups(chat_view);
                let (draft, references_detached) =
                    entry.into_draft_for_workspace(self.workspace.as_deref());
                chat_view.set_chat_draft(draft);
                self.refresh_stash_non_empty();
                chat_view.set_status(Some(restored_status(
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
        let result = crate::prompt_stash::PromptStashStore::from_config_dir()
            .map_err(|error| error.to_string())
            .and_then(|store| store.remove(id).map_err(|error| error.to_string()));
        match result {
            Ok(true) => {
                self.refresh_stash_non_empty();
                let entries = crate::prompt_stash::PromptStashStore::from_config_dir()
                    .ok()
                    .and_then(|store| store.list().ok())
                    .unwrap_or_default();
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

fn restored_status(base: &str, references_detached: bool) -> String {
    if references_detached {
        format!(
            "{base}; workspace references were detached because the stash came from another workspace"
        )
    } else {
        base.to_string()
    }
}
