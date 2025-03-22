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
    if matches!(
        key_event,
        KeyEvent {
            key: KeyCode::Char('c'),
            modifiers: KeyModifiers::CTRL
        }
    ) {
        return Some(MintakaInputEvent::Quit);
    }

    match key_event.key {
        wezterm_term::KeyCode::Char('a') => Some(MintakaInputEvent::ToggleAutofocus),
        wezterm_term::KeyCode::UpArrow => Some(MintakaInputEvent::FocusProcessUp),
        wezterm_term::KeyCode::DownArrow => Some(MintakaInputEvent::FocusProcessDown),
        wezterm_term::KeyCode::Char('r') => Some(MintakaInputEvent::RestartProcess),
        _ => None,
    }
}
