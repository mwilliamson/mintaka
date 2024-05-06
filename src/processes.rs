use std::sync::{Arc, Mutex};

use portable_pty::PtySystem;
use wezterm_term::TerminalSize;

use crate::config::ProcessConfig;

pub(crate) struct Processes {
    pty_system: Box<dyn PtySystem>,

    pty_size: portable_pty::PtySize,

    processes: Vec<Process>,

    pub(crate) focused_process_index: usize,
}

impl Processes {
    pub(crate) fn new() -> Self {
        let pty_system = portable_pty::native_pty_system();
        let pty_size = portable_pty::PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        Self { pty_system, pty_size, processes: Vec::new(), focused_process_index: 0 }
    }

    pub(crate) fn start_process(
        &mut self,
        process_config: ProcessConfig,
    ) -> Result<(), ProcessError> {
        let pty_command = Self::process_config_to_pty_command(&process_config)?;

        let pty_pair = self.pty_system.openpty(self.pty_size).unwrap();

        let child_process = pty_pair.slave.spawn_command(pty_command).unwrap();
        std::mem::drop(pty_pair.slave);

        let child_process_writer = pty_pair.master.take_writer().unwrap();
        let terminal = Arc::new(Mutex::new(Self::create_process_terminal(child_process_writer)));

        let child_process_reader = pty_pair.master.try_clone_reader().unwrap();
        Self::spawn_process_reader(child_process_reader, Arc::clone(&terminal));

        let process = Process {
            name: process_config.name.unwrap_or_else(|| process_config.command.join(" ")),
            _child_process: child_process,
            terminal,
            pty_master: pty_pair.master,
        };
        self.processes.push(process);

        Ok(())
    }

    fn process_config_to_pty_command(process_config: &ProcessConfig) -> Result<portable_pty::CommandBuilder, ProcessError> {
        let executable = process_config.command.first()
            .ok_or(ProcessError::ProcessConfigMissingCommand)?;
        let mut pty_command = portable_pty::CommandBuilder::new(executable);

        pty_command.args(process_config.command.iter().skip(1));

        let current_dir = std::env::current_dir().map_err(ProcessError::GetCurrentDirFailed)?;
        pty_command.cwd(current_dir);

        Ok(pty_command)
    }

    fn create_process_terminal(writer: Box<dyn std::io::Write + Send>) -> wezterm_term::Terminal {
        let terminal_size = wezterm_term::TerminalSize::default();
        let terminal_config = Arc::new(ProcessTerminal);
        wezterm_term::Terminal::new(
            terminal_size,
            terminal_config,
            "Mintaka",
            "1.0.0",
            Box::new(writer),
        )
    }

    fn spawn_process_reader(
        mut reader: Box<dyn std::io::Read + Send>,
        terminal: Arc<Mutex<wezterm_term::Terminal>>,
    ) {
        std::thread::spawn(move || {
            let mut bytes = vec![0; 256];
            loop {
                let bytes_read = reader.read(&mut bytes).unwrap();
                if bytes_read == 0 {
                    break;
                }
                terminal.lock().unwrap().advance_bytes(&bytes[..bytes_read]);
            }
        });
    }

    pub(crate) fn processes(&self) -> &[Process] {
        &self.processes
    }

    pub(crate) fn lines(&self) -> Vec<wezterm_term::Line> {
        let terminal = self.processes[self.focused_process_index].terminal.lock().unwrap();
        terminal.screen().lines_in_phys_range(0..100)
    }

    pub(crate) fn move_focus_up(&mut self) {
        if self.focused_process_index > 0 {
            self.focused_process_index -= 1;
        } else {
            self.focused_process_index = self.processes.len() - 1;
        }
    }

    pub(crate) fn move_focus_down(&mut self) {
        if self.focused_process_index + 1 < self.processes.len() {
            self.focused_process_index += 1;
        } else {
            self.focused_process_index = 0;
        }
    }

    pub(crate) fn resize(&mut self, size: (usize, usize)) {
        self.pty_size.cols = size.0 as u16;
        self.pty_size.rows = size.1 as u16;
        for process in &self.processes {
            process.pty_master.resize(self.pty_size).unwrap();
            let mut terminal = process.terminal.lock().unwrap();
            let dpi = terminal.get_size().dpi;
            terminal.resize(TerminalSize {
                rows: self.pty_size.rows as usize,
                cols: self.pty_size.cols as usize,
                pixel_width: self.pty_size.pixel_width as usize,
                pixel_height: self.pty_size.pixel_height as usize,
                dpi,
            });
        }
    }
}

pub(crate) struct Process {
    pub(crate) name: String,
    _child_process: Box<dyn portable_pty::Child>,
    terminal: Arc<Mutex<wezterm_term::Terminal>>,
    pty_master: Box<dyn portable_pty::MasterPty>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum ProcessError {
    ProcessConfigMissingCommand,

    GetCurrentDirFailed(std::io::Error),
}


#[derive(Debug)]
struct ProcessTerminal;

impl wezterm_term::TerminalConfiguration for ProcessTerminal {
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        wezterm_term::color::ColorPalette::default()
    }
}
