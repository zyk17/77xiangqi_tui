use crate::service::SlashCommand;

#[derive(Debug, Clone, Default)]
pub struct InputState {
    buffer: String,
    cursor: usize,
    suggestion_index: usize,
    suggestion_seed: Option<String>,
}

impl InputState {
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn text(&self) -> &str {
        &self.buffer
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.buffer = text.into();
        self.cursor = self.buffer.len();
        self.reset_completion();
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.reset_completion();
    }

    pub fn take_text(&mut self) -> String {
        let text = self.buffer.clone();
        self.clear();
        text
    }

    pub fn insert_char(&mut self, ch: char) {
        self.buffer.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.reset_completion();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if let Some((start, _)) = self.buffer[..self.cursor].char_indices().last() {
            self.buffer.drain(start..self.cursor);
            self.cursor = start;
            self.reset_completion();
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
        self.reset_completion();
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
        self.suggestions_for(self.buffer.as_str())
    }

    pub fn autocomplete_next(&mut self) {
        if !self.buffer.starts_with('/') {
            return;
        }
        let seed = self
            .suggestion_seed
            .clone()
            .unwrap_or_else(|| self.buffer.to_ascii_lowercase());
        let suggestions = self.suggestions_for(&seed);
        if suggestions.is_empty() {
            return;
        }
        let pick = suggestions[self.suggestion_index % suggestions.len()];
        self.buffer = pick.name().to_string();
        self.cursor = self.buffer.len();
        self.suggestion_seed = Some(seed);
        self.suggestion_index = (self.suggestion_index + 1) % suggestions.len();
    }

    fn suggestions_for(&self, keyword: &str) -> Vec<SlashCommand> {
        if !self.buffer.starts_with('/') {
            return Vec::new();
        }
        SlashCommand::ALL
            .into_iter()
            .filter(|command| command.name().starts_with(keyword))
            .collect()
    }

    fn reset_completion(&mut self) {
        self.suggestion_index = 0;
        self.suggestion_seed = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autocomplete_cycles_slash_matches() {
        let mut input = InputState::default();
        input.set_text("/n");
        input.autocomplete_next();
        assert_eq!(input.text(), "/new");
        input.autocomplete_next();
        assert_eq!(input.text(), "/next");
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
}
