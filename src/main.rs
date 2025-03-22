use std::sync::{Arc, Mutex};

use termwiz::input::{InputEvent, KeyEvent};
use ui::MintakaUi;
use wezterm_term::{KeyCode, KeyModifiers};

use crate::processes::Processes;

mod cli;
mod config;
mod process_statuses;
mod processes;
mod ui;

fn main() {
    let config = cli::load_config().unwrap();

    let mut ui = MintakaUi::new();

    let mut processes = Processes::new(ui.waker());
    for process_config in config.processes {
        processes.start_process(process_config).unwrap();
    }
    let processes = Arc::new(Mutex::new(processes));

    loop {
        {
            let mut processes_locked = processes.lock().unwrap();
            processes_locked.do_work().unwrap();
        }

        ui.render(&processes);

        match ui.poll_input().unwrap() {
            Some(InputEvent::Key(key_event)) => {
                if matches!(
                    key_event,
                    KeyEvent {
                        key: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CTRL
                    }
                ) {
                    return;
                }

                match key_event.key {
                    wezterm_term::KeyCode::UpArrow => {
                        let mut processes = processes.lock().unwrap();
                        processes.disable_autofocus();
                        processes.move_focus_up();
                    }
                    wezterm_term::KeyCode::DownArrow => {
                        let mut processes = processes.lock().unwrap();
                        processes.disable_autofocus();
                        processes.move_focus_down();
                    }
                    wezterm_term::KeyCode::Char('a') => {
                        let mut processes = processes.lock().unwrap();
                        processes.toggle_autofocus();
                    }
                    wezterm_term::KeyCode::Char('r') => {
                        let mut processes = processes.lock().unwrap();
                        processes.restart_focused();
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
