use termwiz::input::KeyEvent;
use wezterm_term::{KeyCode, KeyModifiers};

use crate::{Processes, processes::MintakaMode};

pub(crate) enum MintakaInputEvent {
    ToggleAutofocus,
    FocusProcessUp,
    FocusProcessDown,
    ScrollUp,
    ScrollDown,
    LeaveHistory,
    RestartProcess,
    EnterProcess,
    LeaveProcess,
    Quit,
    // TODO: other events?
    SendToFocusedProcess(KeyEvent),
}

pub(super) fn describe(processes: &Processes) -> Vec<(&str, &str)> {
    match processes.mode() {
        crate::processes::MintakaMode::Main => {
            let autofocus_str = if processes.autofocus_enabled() {
                "Autofocus (On) "
            } else {
                "Autofocus (Off)"
            };

            vec![
                (" a", autofocus_str),
                ("↑↓", "Focus process"),
                ("PgUp", "Scroll up"),
                (" r", "Restart process"),
                ("^e", "Enter process"),
                ("^c", "Quit"),
            ]
        }

        crate::processes::MintakaMode::ForwardInputToFocusedProcess => {
            vec![("^e", "Leave process")]
        }

        crate::processes::MintakaMode::History => {
            vec![
                ("^e", "Leave history"),
                ("PgUp", "Scroll up"),
                ("PgDn", "Scroll down"),
            ]
        }
    }
}

pub(super) fn read_key_event(key_event: KeyEvent, mode: MintakaMode) -> Option<MintakaInputEvent> {
    match mode {
        MintakaMode::Main => match key_event {
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
                key: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
            } => Some(MintakaInputEvent::ScrollUp),

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
        },

        MintakaMode::ForwardInputToFocusedProcess => match key_event {
            KeyEvent {
                key: KeyCode::Char('e'),
                modifiers: KeyModifiers::CTRL,
            } => Some(MintakaInputEvent::LeaveProcess),

            _ => Some(MintakaInputEvent::SendToFocusedProcess(key_event)),
        },

        MintakaMode::History => match key_event {
            KeyEvent {
                key: KeyCode::Char('e'),
                modifiers: KeyModifiers::CTRL,
            } => Some(MintakaInputEvent::LeaveHistory),

            KeyEvent {
                key: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
            } => Some(MintakaInputEvent::ScrollUp),

            KeyEvent {
                key: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
            } => Some(MintakaInputEvent::ScrollDown),

            _ => None,
        },
    }
}
