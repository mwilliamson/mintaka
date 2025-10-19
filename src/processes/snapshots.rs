use wezterm_term::Screen;

use super::scroll::ScrollDirection;

pub(super) struct ProcessSnapshot {
    line_index: usize,

    screen: Option<Screen>,
}

impl ProcessSnapshot {
    pub(super) fn empty() -> Self {
        Self {
            line_index: 0,
            screen: None,
        }
    }

    pub(super) fn new(line_index: usize, screen: Screen) -> Self {
        Self {
            line_index,
            screen: Some(screen),
        }
    }

    pub(super) fn lines(&self) -> Vec<wezterm_term::Line> {
        if let Some(screen) = &self.screen {
            screen.lines_in_phys_range(self.line_index..(self.line_index + screen.physical_rows))
        } else {
            vec![]
        }
    }

    pub(super) fn scroll(&mut self, direction: ScrollDirection) {
        if let Some(screen) = &self.screen {
            let page_scroll_distance = screen.physical_rows / 2;

            match direction {
                ScrollDirection::PageUp => {
                    self.scroll_up(page_scroll_distance);
                }
                ScrollDirection::PageDown => {
                    self.scroll_down(page_scroll_distance);
                }
                ScrollDirection::LineUp => {
                    self.scroll_up(1);
                }
                ScrollDirection::LineDown => {
                    self.scroll_down(1);
                }
            }
        }
    }

    fn scroll_up(&mut self, lines: usize) {
        self.line_index = self.line_index.saturating_sub(lines);
    }

    fn scroll_down(&mut self, lines: usize) {
        if let Some(screen) = &self.screen {
            self.line_index = self
                .line_index
                .saturating_add(lines)
                .min(screen.phys_row(0));
        }
    }
}
