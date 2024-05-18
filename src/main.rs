use std::sync::{Arc, Mutex};

use ratatui::backend::TermwizBackend;
use termwiz::{caps::ProbeHints, input::{InputEvent, KeyEvent}, surface::Change, terminal::{buffered::BufferedTerminal, SystemTerminal, Terminal}};
use ui::render_ui;
use wezterm_term::{KeyCode, KeyModifiers};

use crate::processes::Processes;

mod cli;
mod config;
mod processes;
mod process_types;
mod ui;

fn main() {
    let config = cli::load_config().unwrap();

    let terminal_capabilities = termwiz::caps::Capabilities::new_with_hints(ProbeHints::new_from_env().mouse_reporting(Some(false))).unwrap();
    let mut terminal = SystemTerminal::new(terminal_capabilities).unwrap();
    terminal.set_raw_mode().unwrap();
    terminal.enter_alternate_screen().unwrap();
    let terminal_waker = terminal.waker();
    let buffered_terminal = BufferedTerminal::new(terminal).unwrap();

    let mut terminal = ratatui::Terminal::new(TermwizBackend::with_buffered_terminal(buffered_terminal)).unwrap();

    let mut processes = Processes::new(terminal_waker);
    for process_config in config.processes {
        processes.start_process(process_config).unwrap();
    }
    let processes = Arc::new(Mutex::new(processes));

    loop {
        {
            let mut processes_locked = processes.lock().unwrap();
            processes_locked.do_work().unwrap();
        }

        render_ui(&processes, &mut terminal);

        let buffered_terminal = terminal.backend_mut().buffered_terminal_mut();
        match buffered_terminal.terminal().poll_input(None).unwrap() {
            Some(InputEvent::Resized { rows, cols }) => {
                // FIXME: this is working around a bug where we don't realize
                // that we should redraw everything on resize in BufferedTerminal.
                buffered_terminal.add_change(Change::ClearScreen(Default::default()));
                buffered_terminal.resize(cols, rows);
            }
            Some(input) => {
                if let InputEvent::Key(key_event) = input {
                    if matches!(
                        key_event,
                        KeyEvent { key: KeyCode::Char('q'), .. } |
                        KeyEvent { key: KeyCode::Char('c'), modifiers: KeyModifiers::CTRL}
                    ) {
                        return;
                    }

                    match key_event.key {
                        wezterm_term::KeyCode::UpArrow => {
                            let mut processes = processes.lock().unwrap();
                            processes.move_focus_up();
                        },
                        wezterm_term::KeyCode::DownArrow => {
                            let mut processes = processes.lock().unwrap();
                            processes.move_focus_down();
                        },
                        _ => {},
                    }
                }
            },
            None => {}
        }
    }
}
