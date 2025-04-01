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

        let mode = {
            let processes = processes.lock().unwrap();
            processes.mode()
        };

        match ui.poll_input(mode).unwrap() {
            Some(MintakaInputEvent::Quit) => {
                let mut processes = processes.lock().unwrap();
                processes.stop_all();
                return;
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
            Some(MintakaInputEvent::ScrollUp) => {
                let mut processes = processes.lock().unwrap();
                processes.scroll(ScrollDirection::Up);
            }
            Some(MintakaInputEvent::ScrollDown) => {
                let mut processes = processes.lock().unwrap();
                processes.scroll(ScrollDirection::Down);
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
            Some(MintakaInputEvent::LeaveHistory) => {
                let mut processes = processes.lock().unwrap();
                processes.leave_history();
            }
            None => {}
        }
    }
}
