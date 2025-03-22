use termwiz::input::KeyEvent;
use wezterm_term::{KeyCode, KeyModifiers};

use crate::Processes;

pub(crate) enum MintakaInputEvent {
    ToggleAutofocus,
    FocusProcessUp,
    FocusProcessDown,
    RestartProcess,
    EnterProcess,
    LeaveProcess,
    Quit,
    // TODO: other events?
    SendToFocusedProcess(KeyEvent),
}

pub(super) fn describe(processes: &Processes) -> Vec<(&str, &str)> {
    if processes.entered() {
        vec![("^e", "Leave process")]
    } else {
        let autofocus_str = if processes.autofocus_enabled() {
            "Autofocus (On) "
        } else {
            "Autofocus (Off)"
        };

        vec![
            (" a", autofocus_str),
            ("↑↓", "Focus process"),
            (" r", "Restart process"),
            ("^e", "Enter process"),
            ("^c", "Quit"),
        ]
    }
}

pub(super) fn read_key_event(key_event: KeyEvent, entered: bool) -> Option<MintakaInputEvent> {
    if entered {
        match key_event {
            KeyEvent {
                key: KeyCode::Char('e'),
                modifiers: KeyModifiers::CTRL,
            } => Some(MintakaInputEvent::LeaveProcess),

            _ => Some(MintakaInputEvent::SendToFocusedProcess(key_event)),
        }
    } else {
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
                key: KeyCode::Char('e'),
                modifiers: KeyModifiers::CTRL,
            } => Some(MintakaInputEvent::EnterProcess),

            KeyEvent {
                key: KeyCode::Char('c'),
                modifiers: KeyModifiers::CTRL,
            } => Some(MintakaInputEvent::Quit),

            _ => None,
        }
    }
}
