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

    pub(super) fn text_style(&self) -> Style {
        Style::default().fg(Color::Reset).bg(Color::Reset)
    }

    pub(super) fn highlight_style(&self) -> Style {
        match self {
            MintakaTheme::Light => Style::default().fg(Color::White).bg(Color::Black),
            MintakaTheme::Dark => Style::default().fg(Color::Black).bg(Color::White),
        }
    }
}
