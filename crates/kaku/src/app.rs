use regex_automata::meta::Regex;

use kaku_core::Document;
use kaku_render::{Layout, LayoutLine, LayoutOptions, ThemeName, TocEntry, layout_document};

#[derive(Debug, Clone)]
pub struct AppState {
    pub document: Document,
    pub layout: Layout,
    pub scroll: usize,
    pub viewport_height: usize,
    pub status: String,
    pub toc_open: bool,
    pub toc_selected: usize,
    pub theme: ThemeName,
    pub syntax_highlighting: bool,
    pub search: SearchState,
}

impl AppState {
    pub fn new(
        document: Document,
        width: usize,
        height: usize,
        theme: ThemeName,
        syntax_highlighting: bool,
        toc_open: bool,
    ) -> Self {
        let layout = layout_document(
            &document,
            &LayoutOptions {
                width,
                theme,
                syntax_highlighting,
            },
        );

        Self {
            document,
            layout,
            scroll: 0,
            viewport_height: height.saturating_sub(1),
            status: "Press / to search, t for TOC, q to quit".to_string(),
            toc_open,
            toc_selected: 0,
            theme,
            syntax_highlighting,
            search: SearchState::default(),
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.viewport_height = height.saturating_sub(1);
        self.layout = layout_document(
            &self.document,
            &LayoutOptions {
                width,
                theme: self.theme,
                syntax_highlighting: self.syntax_highlighting,
            },
        );
        self.scroll = self.scroll.min(self.max_scroll());
        self.realign_toc_selection();
    }

    pub fn replace_document(&mut self, document: Document, width: usize) {
        self.document = document;
        self.layout = layout_document(
            &self.document,
            &LayoutOptions {
                width,
                theme: self.theme,
                syntax_highlighting: self.syntax_highlighting,
            },
        );
        self.scroll = self.scroll.min(self.max_scroll());
        self.status = "reloaded".to_string();
        self.apply_search();
        self.realign_toc_selection();
    }

    pub fn visible_lines(&self) -> &[LayoutLine] {
        let end = (self.scroll + self.viewport_height).min(self.layout.lines.len());
        &self.layout.lines[self.scroll..end]
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll = (self.scroll + amount).min(self.max_scroll());
        self.realign_toc_selection();
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_sub(amount);
        self.realign_toc_selection();
    }

    pub fn page_down(&mut self) {
        self.scroll_down(self.viewport_height.saturating_sub(1).max(1));
    }

    pub fn page_up(&mut self) {
        self.scroll_up(self.viewport_height.saturating_sub(1).max(1));
    }

    pub fn go_top(&mut self) {
        self.scroll = 0;
        self.realign_toc_selection();
    }

    pub fn go_bottom(&mut self) {
        self.scroll = self.max_scroll();
        self.realign_toc_selection();
    }

    pub fn toggle_toc(&mut self) {
        self.toc_open = !self.toc_open;
        self.realign_toc_selection();
    }

    pub fn select_next_toc(&mut self) {
        if !self.layout.toc.is_empty() {
            self.toc_selected = (self.toc_selected + 1).min(self.layout.toc.len() - 1);
        }
    }

    pub fn select_prev_toc(&mut self) {
        self.toc_selected = self.toc_selected.saturating_sub(1);
    }

    pub fn jump_to_selected_toc(&mut self) {
        if let Some(entry) = self.layout.toc.get(self.toc_selected) {
            self.scroll = entry.line_index.min(self.max_scroll());
            self.status = format!("jumped to {}", entry.title);
        }
    }

    pub fn toc_entries(&self) -> &[TocEntry] {
        &self.layout.toc
    }

    pub fn apply_search_query(&mut self, query: String) {
        self.search.query = query;
        self.apply_search();
    }

    pub fn next_search_match(&mut self) {
        if self.search.matches.is_empty() {
            self.status = "no matches".to_string();
            return;
        }

        self.search.current = (self.search.current + 1) % self.search.matches.len();
        self.scroll = self.search.matches[self.search.current].min(self.max_scroll());
        self.status = format!(
            "match {} of {}",
            self.search.current + 1,
            self.search.matches.len()
        );
        self.realign_toc_selection();
    }

    pub fn previous_search_match(&mut self) {
        if self.search.matches.is_empty() {
            self.status = "no matches".to_string();
            return;
        }

        self.search.current = if self.search.current == 0 {
            self.search.matches.len() - 1
        } else {
            self.search.current - 1
        };
        self.scroll = self.search.matches[self.search.current].min(self.max_scroll());
        self.status = format!(
            "match {} of {}",
            self.search.current + 1,
            self.search.matches.len()
        );
        self.realign_toc_selection();
    }

    pub fn first_visible_link(&self) -> Option<usize> {
        self.visible_lines()
            .iter()
            .flat_map(|line| line.link_indices.iter().copied())
            .next()
    }

    fn apply_search(&mut self) {
        if self.search.query.is_empty() {
            self.search.matches.clear();
            self.search.current = 0;
            self.status = "search cleared".to_string();
            return;
        }

        let pattern = format!("(?i){}", self.search.query);
        let regex = match Regex::new(&pattern) {
            Ok(regex) => regex,
            Err(error) => {
                self.status = format!("invalid search: {error}");
                return;
            }
        };

        self.search.matches = self
            .layout
            .lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                if regex.is_match(line.plain_text.as_bytes()) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();
        self.search.current = 0;

        if let Some(line) = self.search.matches.first().copied() {
            self.scroll = line.min(self.max_scroll());
            self.status = format!("{} matches", self.search.matches.len());
            self.realign_toc_selection();
        } else {
            self.status = "no matches".to_string();
        }
    }

    fn max_scroll(&self) -> usize {
        self.layout
            .lines
            .len()
            .saturating_sub(self.viewport_height.max(1))
    }

    fn realign_toc_selection(&mut self) {
        if self.layout.toc.is_empty() {
            self.toc_selected = 0;
            return;
        }

        if let Some((index, _)) = self
            .layout
            .toc
            .iter()
            .enumerate()
            .rev()
            .find(|(_, entry)| entry.line_index <= self.scroll)
        {
            self.toc_selected = index;
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub matches: Vec<usize>,
    pub current: usize,
}

#[cfg(test)]
mod tests {
    use kaku_core::parse_document;
    use kaku_render::ThemeName;

    use super::AppState;

    #[test]
    fn search_moves_scroll() {
        let doc = parse_document("# One\n\nHello\n\n# Two\n\nWorld\n");
        let mut app = AppState::new(doc, 80, 3, ThemeName::Dark, false, false);
        app.apply_search_query("world".to_string());

        assert_eq!(app.search.matches.len(), 1);
        assert!(app.scroll > 0);
    }

    #[test]
    fn toc_jump_uses_selected_entry() {
        let doc = parse_document("# One\n\n## Two\n");
        let mut app = AppState::new(doc, 80, 2, ThemeName::Dark, false, true);
        app.select_next_toc();
        app.jump_to_selected_toc();

        assert!(app.scroll > 0);
    }
}
