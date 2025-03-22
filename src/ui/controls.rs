use termwiz::input::KeyEvent;
use wezterm_term::{KeyCode, KeyModifiers};

use crate::Processes;

pub(crate) enum MintakaInputEvent {
    ToggleAutofocus,
    FocusProcessUp,
    FocusProcessDown,
    RestartProcess,
    Quit,
}

pub(super) fn describe(processes: &Processes) -> [(&str, &str); 4] {
    let autofocus_str = if processes.autofocus() {
        "Autofocus (On) "
    } else {
        "Autofocus (Off)"
    };

    [
        (" a", autofocus_str),
        ("↑↓", "Focus process"),
        (" r", "Restart process"),
        ("^c", "Quit"),
    ]
}

pub(super) fn read_key_event(key_event: KeyEvent) -> Option<MintakaInputEvent> {
    match key_event {
        KeyEvent {
            key: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
        } => Some(MintakaInputEvent::ToggleAutofocus),

        KeyEvent {
            key: KeyCode::UpArrow,
            modifiers: KeyModifiers::NONE,
        } => Some(MintakaInputEvent::FocusProcessUp),

        KeyEvent {
            key: KeyCode::DownArrow,
            modifiers: KeyModifiers::NONE,
        } => Some(MintakaInputEvent::FocusProcessDown),

        KeyEvent {
            key: KeyCode::Char('r'),
            modifiers: KeyModifiers::NONE,
        } => Some(MintakaInputEvent::RestartProcess),

        KeyEvent {
            key: KeyCode::Char('c'),
            modifiers: KeyModifiers::CTRL,
        } => Some(MintakaInputEvent::Quit),

        _ => None,
    }
}
