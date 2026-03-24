use kaku_core::Document;
use kaku_render::{Layout, LayoutLine, LayoutOptions, ThemeName, TocEntry, layout_document};

#[derive(Debug, Clone)]
pub struct AppState {
    pub document: Document,
    pub layout: Layout,
    pub source_name: String,
    pub scroll: usize,
    pub terminal_width: usize,
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
        source_name: String,
        width: usize,
        height: usize,
        theme: ThemeName,
        syntax_highlighting: bool,
        toc_open: bool,
    ) -> Self {
        let mut app = Self {
            document,
            layout: Layout {
                lines: Vec::new(),
                toc: Vec::new(),
            },
            source_name,
            scroll: 0,
            terminal_width: width,
            viewport_height: height.saturating_sub(1),
            status: "j/k move  / search  ? help  q quit".to_string(),
            toc_open,
            toc_selected: 0,
            theme,
            syntax_highlighting,
            search: SearchState::default(),
        };
        app.relayout();
        app
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.terminal_width = width;
        self.viewport_height = height.saturating_sub(1);
        self.relayout();
        self.scroll = self.scroll.min(self.max_scroll());
        self.realign_toc_selection();
    }

    pub fn replace_document(&mut self, document: Document, width: usize) {
        self.document = document;
        self.terminal_width = width;
        self.relayout();
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
        self.relayout();
        self.scroll = self.scroll.min(self.max_scroll());
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

        let query = self.search.query.to_lowercase();

        self.search.matches = self
            .layout
            .lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                if line.plain_text.to_lowercase().contains(&query) {
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

    fn relayout(&mut self) {
        self.layout = layout_document(
            &self.document,
            &LayoutOptions {
                width: self.body_width(),
                theme: self.theme,
                syntax_highlighting: self.syntax_highlighting,
            },
        );
    }

    fn body_width(&self) -> usize {
        if self.toc_width() > 0 {
            self.frame_width()
                .saturating_sub(self.toc_width())
                .saturating_sub(self.frame_gap())
                .max(24)
        } else {
            self.frame_width()
        }
    }

    pub fn toc_width(&self) -> usize {
        let frame_width = self.frame_width();
        if self.toc_open && frame_width >= 64 {
            frame_width.saturating_div(4).clamp(18, 24)
        } else {
            0
        }
    }

    pub fn frame_width(&self) -> usize {
        let edge_margin = if self.terminal_width >= 96 { 12 } else { 4 };
        let usable = self.terminal_width.saturating_sub(edge_margin * 2).max(1);
        let minimum = self.terminal_width.clamp(1, 24);
        let width = if usable < minimum {
            self.terminal_width.max(1)
        } else {
            usable
        };
        width.clamp(minimum, 84)
    }

    pub fn frame_x(&self) -> usize {
        self.terminal_width
            .saturating_sub(self.frame_width())
            .saturating_div(2)
    }

    pub fn body_x(&self) -> usize {
        self.frame_x()
            + if self.toc_width() > 0 {
                self.toc_width() + self.frame_gap()
            } else {
                0
            }
    }

    pub fn frame_gap(&self) -> usize {
        if self.toc_width() > 0 { 2 } else { 0 }
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
        let mut app = AppState::new(
            doc,
            "test.md".to_string(),
            80,
            3,
            ThemeName::Dark,
            false,
            false,
        );
        app.apply_search_query("world".to_string());

        assert_eq!(app.search.matches.len(), 1);
        assert!(app.scroll > 0);
    }

    #[test]
    fn toc_jump_uses_selected_entry() {
        let doc = parse_document("# One\n\n## Two\n");
        let mut app = AppState::new(
            doc,
            "test.md".to_string(),
            80,
            2,
            ThemeName::Dark,
            false,
            true,
        );
        app.select_next_toc();
        app.jump_to_selected_toc();

        assert!(app.scroll > 0);
    }

    #[test]
    fn toggling_toc_reflows_for_narrower_body() {
        let doc = parse_document("0123456789012345678901234567890123456789");
        let mut app = AppState::new(
            doc,
            "test.md".to_string(),
            40,
            10,
            ThemeName::Dark,
            false,
            false,
        );
        let without_toc = app.layout.lines.len();

        app.toggle_toc();

        assert!(app.layout.lines.len() >= without_toc);
        assert_eq!(app.toc_width(), 0);
        assert_eq!(app.body_width(), 32);
    }

    #[test]
    fn toc_keeps_a_visible_body_on_small_terminals() {
        let doc = parse_document("# One\n\nbody");
        let app = AppState::new(
            doc,
            "test.md".to_string(),
            12,
            10,
            ThemeName::Dark,
            false,
            true,
        );

        assert_eq!(app.toc_width(), 0);
        assert_eq!(app.body_width(), 12);
    }
}
