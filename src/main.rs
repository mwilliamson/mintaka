use std::sync::{Arc, Mutex};

use processes::ScrollDirection;
use ui::{MintakaInputEvent, MintakaUi};

use crate::processes::Processes;

mod cli;
mod config;
mod process_statuses;
mod processes;
mod ui;

fn main() {
    let config = cli::load_config().unwrap();

    let mut ui = MintakaUi::new();

    let processes = Processes::new(ui.waker(), config.processes);
    let processes = Arc::new(Mutex::new(processes));

    // Render once before processes are spawned to get the initial area
    // available for output.
    ui.render(&processes);

    loop {
        {
            let mut processes_locked = processes.lock().unwrap();
            processes_locked.do_work().unwrap();

            if processes_locked.is_stopped() {
                return;
            }
        }

        ui.render(&processes);

        let mode = {
            let processes = processes.lock().unwrap();
            processes.mode()
        };

        match ui.poll_input(mode).unwrap() {
            Some(MintakaInputEvent::Quit) => {
                let mut processes = processes.lock().unwrap();
                processes.stop_all();
            }
            Some(MintakaInputEvent::FocusProcessUp) => {
                let mut processes = processes.lock().unwrap();
                processes.disable_autofocus();
                processes.move_focus_up();
            }
            Some(MintakaInputEvent::FocusProcessDown) => {
                let mut processes = processes.lock().unwrap();
                processes.disable_autofocus();
                processes.move_focus_down();
            }
            Some(MintakaInputEvent::ScrollPageUp) => {
                let mut processes = processes.lock().unwrap();
                processes.scroll(ScrollDirection::PageUp);
            }
            Some(MintakaInputEvent::ScrollPageDown) => {
                let mut processes = processes.lock().unwrap();
                processes.scroll(ScrollDirection::PageDown);
            }
            Some(MintakaInputEvent::ScrollLineUp) => {
                let mut processes = processes.lock().unwrap();
                processes.scroll(ScrollDirection::LineUp);
            }
            Some(MintakaInputEvent::ScrollLineDown) => {
                let mut processes = processes.lock().unwrap();
                processes.scroll(ScrollDirection::LineDown);
            }
            Some(MintakaInputEvent::ToggleAutofocus) => {
                let mut processes = processes.lock().unwrap();
                processes.toggle_autofocus();
            }
            Some(MintakaInputEvent::RestartProcess) => {
                let mut processes = processes.lock().unwrap();
                processes.restart_focused();
            }
            Some(MintakaInputEvent::EnterProcess) => {
                let mut processes = processes.lock().unwrap();
                processes.forward_input_to_focused_process();
            }
            Some(MintakaInputEvent::LeaveProcess) => {
                let mut processes = processes.lock().unwrap();
                processes.enter_main_mode();
            }
            Some(MintakaInputEvent::SendToFocusedProcess(key_event)) => {
                let mut processes = processes.lock().unwrap();
                processes.send_input(key_event);
            }
            Some(MintakaInputEvent::EnterHistory) => {
                let mut processes = processes.lock().unwrap();
                processes.enter_history();
            }
            Some(MintakaInputEvent::LeaveHistory) => {
                let mut processes = processes.lock().unwrap();
                processes.leave_history();
            }
            None => {}
        }
    }
}
