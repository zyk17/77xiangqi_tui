use crate::service::SlashCommand;

#[derive(Debug, Clone, Default)]
pub struct InputState {
    buffer: String,
    cursor: usize,
    slash_pick: Option<usize>,
    command_history: Vec<String>,
    /// `0..len` 浏览历史；`len` 表示在最新行（可输入新命令）。
    history_index: usize,
    /// 从最新行按 ↑ 前未提交的草稿，按 ↓ 回到最新行时恢复。
    history_draft: Option<String>,
}

impl InputState {
    pub fn text(&self) -> &str {
        &self.buffer
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn slash_menu_open(&self) -> bool {
        self.buffer.starts_with('/') && !self.suggestions().is_empty()
    }

    pub fn slash_pick_index(&self) -> usize {
        self.slash_pick.unwrap_or(0)
    }

    pub fn selected_slash_command(&self) -> Option<SlashCommand> {
        let idx = self.slash_pick_index();
        self.suggestions().get(idx).copied()
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.buffer = text.into();
        self.cursor = self.buffer.len();
        self.sync_slash_pick();
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.slash_pick = None;
        self.history_index = self.command_history.len();
        self.history_draft = None;
    }

    pub fn take_text(&mut self) -> String {
        let text = self.buffer.clone();
        self.clear();
        text
    }

    pub fn insert_char(&mut self, ch: char) {
        self.buffer.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.sync_slash_pick();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if let Some((start, _)) = self.buffer[..self.cursor].char_indices().last() {
            self.buffer.drain(start..self.cursor);
            self.cursor = start;
            self.sync_slash_pick();
        }
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        let next = self
            .buffer
            .get(self.cursor..)
            .and_then(|tail| tail.char_indices().nth(1).map(|(idx, _)| self.cursor + idx))
            .unwrap_or(self.buffer.len());
        self.buffer.drain(self.cursor..next);
        self.sync_slash_pick();
    }

    pub fn commit_command_history(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        self.history_draft = None;
        if self.command_history.last().map(String::as_str) == Some(line) {
            self.history_index = self.command_history.len();
            return;
        }
        self.command_history.push(line.to_string());
        self.history_index = self.command_history.len();
    }

    pub fn history_prev(&mut self) -> bool {
        if self.command_history.is_empty() {
            return false;
        }
        if self.history_index == self.command_history.len() {
            self.history_draft = if self.buffer.is_empty() {
                None
            } else {
                Some(self.buffer.clone())
            };
            self.history_index = self.command_history.len() - 1;
        } else if self.history_index > 0 {
            self.history_index -= 1;
        } else {
            return false;
        }
        self.load_history_entry();
        true
    }

    pub fn history_next(&mut self) -> bool {
        if self.command_history.is_empty() || self.history_index >= self.command_history.len() {
            return false;
        }
        self.history_index += 1;
        if self.history_index >= self.command_history.len() {
            self.restore_history_draft();
            return true;
        }
        self.load_history_entry();
        true
    }

    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if let Some((start, _)) = self.buffer[..self.cursor].char_indices().last() {
            self.cursor = start;
        }
    }

    pub fn move_right(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        self.cursor = self
            .buffer
            .get(self.cursor..)
            .and_then(|tail| tail.char_indices().nth(1).map(|(idx, _)| self.cursor + idx))
            .unwrap_or(self.buffer.len());
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.buffer.len();
    }

    pub fn suggestions(&self) -> Vec<SlashCommand> {
        if !self.buffer.starts_with('/') {
            return Vec::new();
        }
        let keyword = self.buffer.to_ascii_lowercase();
        SlashCommand::ALL
            .into_iter()
            .filter(|command| command.name().starts_with(&keyword))
            .collect()
    }

    pub fn move_slash_pick(&mut self, delta: isize) {
        let list = self.suggestions();
        if list.is_empty() {
            self.slash_pick = None;
            return;
        }
        let current = self.slash_pick.unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(list.len() as isize) as usize;
        self.slash_pick = Some(next);
    }

    pub fn apply_slash_pick_to_buffer(&mut self) {
        let Some(cmd) = self.selected_slash_command() else {
            return;
        };
        self.buffer = cmd.name().to_string();
        self.cursor = self.buffer.len();
        self.sync_slash_pick();
    }

    /// `/` 命令补全（Tab / →）；无匹配时返回 `false`。
    pub fn try_slash_complete(&mut self) -> bool {
        if !self.buffer.starts_with('/') || self.suggestions().is_empty() {
            return false;
        }
        self.apply_slash_pick_to_buffer();
        true
    }

    fn restore_history_draft(&mut self) {
        self.history_index = self.command_history.len();
        if let Some(draft) = self.history_draft.take() {
            self.buffer = draft;
        } else {
            self.buffer.clear();
        }
        self.cursor = self.buffer.len();
        self.sync_slash_pick();
    }

    fn load_history_entry(&mut self) {
        if let Some(entry) = self.command_history.get(self.history_index) {
            self.buffer = entry.clone();
            self.cursor = self.buffer.len();
            self.sync_slash_pick();
        }
    }

    fn sync_slash_pick(&mut self) {
        if !self.buffer.starts_with('/') {
            self.slash_pick = None;
            return;
        }
        let list = self.suggestions();
        if list.is_empty() {
            self.slash_pick = None;
            return;
        }
        let idx = self.slash_pick.unwrap_or(0).min(list.len() - 1);
        self.slash_pick = Some(idx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_menu_lists_all_matching() {
        let mut input = InputState::default();
        input.set_text("/");
        assert_eq!(input.suggestions().len(), SlashCommand::ALL.len());
    }

    #[test]
    fn apply_slash_pick_fills_buffer() {
        let mut input = InputState::default();
        input.set_text("/n");
        input.move_slash_pick(0);
        input.apply_slash_pick_to_buffer();
        assert_eq!(input.text(), "/new");
    }

    #[test]
    fn slash_pick_moves_with_filter() {
        let mut input = InputState::default();
        input.set_text("/n");
        let n = input.suggestions().len();
        assert!(n >= 2);
        input.move_slash_pick(1);
        assert_eq!(input.slash_pick_index(), 1);
    }

    #[test]
    fn cursor_insert_and_backspace_work() {
        let mut input = InputState::default();
        input.insert_char('a');
        input.insert_char('c');
        input.move_left();
        input.insert_char('b');
        assert_eq!(input.text(), "abc");
        input.backspace();
        assert_eq!(input.text(), "ac");
    }

    #[test]
    fn try_slash_complete_fills_buffer() {
        let mut input = InputState::default();
        input.set_text("/n");
        assert!(input.try_slash_complete());
        assert_eq!(input.text(), "/new");
    }

    #[test]
    fn command_history_recall() {
        let mut input = InputState::default();
        input.commit_command_history("h2e2");
        input.commit_command_history("undo");
        assert!(input.history_prev());
        assert_eq!(input.text(), "undo");
        assert!(input.history_prev());
        assert_eq!(input.text(), "h2e2");
        assert!(input.history_next());
        assert_eq!(input.text(), "undo");
        assert!(input.history_next());
        assert!(input.text().is_empty());
    }

    #[test]
    fn edit_while_browsing_does_not_truncate_history() {
        let mut input = InputState::default();
        input.commit_command_history("h2e2");
        input.commit_command_history("undo");
        assert!(input.history_prev());
        input.insert_char('x');
        assert!(input.history_prev());
        assert_eq!(input.text(), "h2e2");
        assert!(input.history_next());
        assert_eq!(input.text(), "undo");
        assert_eq!(input.command_history.len(), 2);
    }

    #[test]
    fn draft_restored_when_returning_to_latest_without_enter() {
        let mut input = InputState::default();
        input.commit_command_history("h2e2");
        input.set_text("h9g7");
        assert!(input.history_prev());
        assert_eq!(input.text(), "h2e2");
        assert!(input.history_next());
        assert_eq!(input.text(), "h9g7");
    }

    #[test]
    fn browse_edit_not_committed_until_enter() {
        let mut input = InputState::default();
        input.commit_command_history("undo");
        assert!(input.history_prev());
        input.set_text("h2e2");
        input.commit_command_history("h2e2");
        assert_eq!(input.command_history, vec!["undo", "h2e2"]);
    }
}
