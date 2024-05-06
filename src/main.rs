use std::time::Duration;

use termwiz::terminal::Terminal;
use wezterm_term::CellAttributes;

use crate::processes::Processes;

mod cli;
mod config;
mod processes;

fn main() {
    let config = cli::load_config().unwrap();

    let mut processes = Processes::new();
    for process_config in config.processes {
        processes.start_process(process_config).unwrap();
    }

    std::thread::sleep(Duration::from_secs(1));

    let lines = processes.lines();
    let real_terminal_capabilities = termwiz::caps::Capabilities::new_from_env().unwrap();
    let mut real_terminal = termwiz::terminal::new_terminal(real_terminal_capabilities).unwrap();
    for line in lines {
        let changes = line.changes(&CellAttributes::blank());
        real_terminal.render(&changes).unwrap();
        real_terminal.render(&[
            termwiz::surface::Change::Text("\r\n".to_owned()),
            termwiz::surface::Change::AllAttributes(CellAttributes::blank()),
        ]).unwrap();
    }
}
