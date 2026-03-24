use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default)]
pub struct PromptState {
    value: String,
}

impl PromptState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn handle_key(&mut self, event: KeyEvent) -> PromptAction {
        match event.code {
            KeyCode::Esc => PromptAction::Cancel,
            KeyCode::Enter => PromptAction::Submit(self.value.clone()),
            KeyCode::Backspace => {
                self.value.pop();
                PromptAction::Continue
            }
            KeyCode::Char('u') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.clear();
                PromptAction::Continue
            }
            KeyCode::Char(ch)
                if !event.modifiers.contains(KeyModifiers::CONTROL)
                    && !event.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.value.push(ch);
                PromptAction::Continue
            }
            _ => PromptAction::Continue,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PromptAction {
    Continue,
    Submit(String),
    Cancel,
}
