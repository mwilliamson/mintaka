use std::time::Duration;

use ratatui::style::{Color, Style};

#[derive(Clone, Copy)]
pub(super) enum MintakaTheme {
    Light,
    Dark,
}

impl MintakaTheme {
    pub(super) fn detect() -> Self {
        let timeout = Duration::from_millis(100);
        let theme = termbg::theme(timeout);
        match theme {
            Ok(termbg::Theme::Light) | Err(_) => MintakaTheme::Light,
            Ok(termbg::Theme::Dark) => MintakaTheme::Dark,
        }
    }

    fn fg_invert(&self) -> Color {
        match self {
            MintakaTheme::Light => Color::White,
            MintakaTheme::Dark => Color::Black,
        }
    }

    fn bg_invert(&self) -> Color {
        match self {
            MintakaTheme::Light => Color::Black,
            MintakaTheme::Dark => Color::White,
        }
    }

    pub(super) fn text_style(&self) -> Style {
        Style::default().fg(Color::Reset).bg(Color::Reset)
    }

    pub(super) fn invert_style(&self) -> Style {
        Style::default().fg(self.fg_invert()).bg(self.bg_invert())
    }
}
