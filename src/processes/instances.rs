use std::{
    sync::{
        Arc, Mutex,
        atomic::{self, AtomicBool},
    },
    time::Duration,
};

use portable_pty::{ExitStatus, PtyPair, PtySize};
use termwiz::{
    escape::{Esc, EscCode, parser::Parser},
    input::KeyEvent,
    terminal::TerminalWaker,
};
use wezterm_term::{CursorPosition, TerminalSize, VisibleRowIndex};

use crate::{
    config::ProcessConfig,
    process_statuses::{LineAnalysis, ProcessStatusAnalyzer},
};

use super::{
    ProcessSnapshot,
    errors::ProcessError,
    statuses::{ProcessStatus, SuccessId},
};

/// The ID of the a process instance, unique within the context of a particular
/// process.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct ProcessInstanceId(u32);

impl ProcessInstanceId {
    /// Create a new process instance ID for a process.
    pub(super) fn new() -> Self {
        Self(0)
    }

    /// Increment the ID, returning the value before the increment.
    pub(super) fn increment(&mut self) -> Self {
        let previous = *self;
        self.0 += 1;
        previous
    }
}

pub(super) struct ProcessInstance {
    terminal: Arc<Mutex<wezterm_term::Terminal>>,
    pty_master: Box<dyn portable_pty::MasterPty>,
    has_terminated: Arc<AtomicBool>,
    process_id: u32,
}

impl ProcessInstance {
    pub(super) fn start(
        process_config: &ProcessConfig,
        pty_pair: PtyPair,
        on_change: TerminalWaker,
        status_tx: std::sync::mpsc::Sender<ProcessStatus>,
        process_instance_id: ProcessInstanceId,
    ) -> Result<Self, ProcessError> {
        let pty_command = Self::process_config_to_pty_command(&process_config)?;

        let child_process = pty_pair
            .slave
            .spawn_command(pty_command)
            .map_err(ProcessError::SpawnCommandFailed)?;
        let process_id = child_process.process_id().expect("Process has no ID");
        std::mem::drop(pty_pair.slave);

        let pty_size = pty_pair.master.get_size().unwrap();
        let child_process_writer = pty_pair.master.take_writer().unwrap();
        let terminal = Arc::new(Mutex::new(Self::create_process_terminal(
            child_process_writer,
            pty_size,
        )));

        let has_terminated = Arc::new(AtomicBool::new(false));

        let child_process_reader = pty_pair.master.try_clone_reader().unwrap();
        Self::spawn_process_reader(
            process_config.process_status_analyzer(),
            child_process,
            child_process_reader,
            Arc::clone(&terminal),
            on_change,
            status_tx,
            Arc::clone(&has_terminated),
            process_instance_id,
        );

        Ok(Self {
            terminal,
            pty_master: pty_pair.master,
            process_id,
            has_terminated,
        })
    }

    fn process_config_to_pty_command(
        process_config: &ProcessConfig,
    ) -> Result<portable_pty::CommandBuilder, ProcessError> {
        let executable = process_config
            .command
            .first()
            .ok_or(ProcessError::ProcessConfigMissingCommand)?;
        let mut pty_command = portable_pty::CommandBuilder::new(executable);

        pty_command.args(process_config.command.iter().skip(1));

        let current_dir = std::env::current_dir().map_err(ProcessError::GetCurrentDirFailed)?;
        let working_directory = match &process_config.working_directory {
            Some(relative_working_directory) => current_dir.join(relative_working_directory),
            None => current_dir,
        };
        pty_command.cwd(working_directory);

        Ok(pty_command)
    }

    fn create_process_terminal(
        writer: Box<dyn std::io::Write + Send>,
        size: PtySize,
    ) -> wezterm_term::Terminal {
        let terminal_size = wezterm_term::TerminalSize {
            rows: size.rows.into(),
            cols: size.cols.into(),
            pixel_width: size.pixel_width.into(),
            pixel_height: size.pixel_height.into(),
            ..Default::default()
        };
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
        process_status_analyzer: ProcessStatusAnalyzer,
        mut child_process: Box<dyn portable_pty::Child>,
        mut reader: Box<dyn std::io::Read + Send>,
        terminal: Arc<Mutex<wezterm_term::Terminal>>,
        on_change: TerminalWaker,
        status_tx: std::sync::mpsc::Sender<ProcessStatus>,
        has_terminated: Arc<AtomicBool>,
        process_instance_id: ProcessInstanceId,
    ) {
        std::thread::spawn(move || {
            let mut bytes = vec![0; 256];
            let mut parser = Parser::new();
            // TODO: Perhaps rather than separately storing the last line, track
            // whether the screen has been cleared and use stable lines in the
            // terminal screen?
            let mut last_line = String::new();
            let mut next_success_id = SuccessId::new(process_instance_id);

            loop {
                let bytes_read = reader.read(&mut bytes).unwrap();
                if bytes_read == 0 {
                    // TODO: handle failure to get exit code properly
                    let exit_code = child_process
                        .wait()
                        .unwrap_or(ExitStatus::with_exit_code(1));

                    has_terminated.store(true, atomic::Ordering::Relaxed);

                    let new_status = ProcessStatus::Exited {
                        exit_code: exit_code.exit_code(),
                    };

                    let _ = status_tx.send(new_status);

                    on_change.wake().unwrap();

                    break;
                }

                let mut actions = Vec::new();

                parser.parse(&bytes[..bytes_read], |action| actions.push(action));

                for action in &actions {
                    // TODO: handle other control codes?
                    match action {
                        termwiz::escape::Action::Print(char) => last_line.push(*char),
                        termwiz::escape::Action::PrintString(string) => last_line.push_str(string),
                        termwiz::escape::Action::Control(
                            termwiz::escape::ControlCode::LineFeed
                            | termwiz::escape::ControlCode::CarriageReturn,
                        )
                        | termwiz::escape::Action::Esc(Esc::Code(EscCode::FullReset)) => {
                            if let Some(line_analysis) =
                                process_status_analyzer.analyze_line(&last_line)
                            {
                                let new_status = match line_analysis {
                                    LineAnalysis::Running => ProcessStatus::Running,
                                    LineAnalysis::Success => {
                                        ProcessStatus::Success(next_success_id.increment())
                                    }
                                    LineAnalysis::Errors { error_count } => {
                                        ProcessStatus::Errors { error_count }
                                    }
                                };
                                let _ = status_tx.send(new_status);
                            }

                            last_line.clear();
                        }
                        _ => {}
                    }
                }

                let mut terminal_locked = terminal.lock().unwrap();
                terminal_locked.perform_actions(actions);

                on_change.wake().unwrap();
            }
        });
    }

    pub(super) fn kill(&mut self) {
        if self.has_terminated.load(atomic::Ordering::Relaxed) {
            return;
        }

        // There is a potential race condition in that the the process might
        // terminate between after having checked whether it has terminated, or
        // that the termination has not yet been handled. However, given this
        // mean the kill happens almost immediately after the process
        // terminates, there should be a low chance that the PID has been
        // reused.

        kill_sigterm(self.process_id);

        let process_id = self.process_id;
        let has_terminated = Arc::clone(&self.has_terminated);

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            if has_terminated.load(atomic::Ordering::Relaxed) {
                return;
            }
            kill_sigkill(process_id);
        });
    }

    pub(super) fn resize(&mut self, pty_size: PtySize) {
        self.pty_master.resize(pty_size).unwrap();
        let mut terminal = self.terminal.lock().unwrap();
        let dpi = terminal.get_size().dpi;
        terminal.resize(TerminalSize {
            rows: pty_size.rows as usize,
            cols: pty_size.cols as usize,
            pixel_width: pty_size.pixel_width as usize,
            pixel_height: pty_size.pixel_height as usize,
            dpi,
        });
    }

    pub(super) fn lines(&self) -> Vec<wezterm_term::Line> {
        let terminal = self.terminal.lock().unwrap();
        terminal
            .screen()
            .lines_in_phys_range(terminal.screen().phys_range(&(0..VisibleRowIndex::MAX)))
    }

    pub(super) fn cursor_position(&self) -> Option<CursorPosition> {
        let terminal = self.terminal.lock().unwrap();
        Some(terminal.cursor_pos())
    }

    pub(super) fn send_input(&self, input: KeyEvent) {
        let mut terminal = self.terminal.lock().unwrap();
        // TODO: handle errors
        let _ = terminal.key_down(input.key, input.modifiers);
        let _ = terminal.key_up(input.key, input.modifiers);
    }

    pub(super) fn snapshot(&self) -> ProcessSnapshot {
        let terminal = self.terminal.lock().unwrap();
        let screen = terminal.screen().clone();

        ProcessSnapshot::new(screen.phys_row(0), screen)
    }
}

#[derive(Debug)]
struct ProcessTerminal;

impl wezterm_term::TerminalConfiguration for ProcessTerminal {
    fn color_palette(&self) -> wezterm_term::color::ColorPalette {
        wezterm_term::color::ColorPalette::default()
    }
}

#[cfg(unix)]
fn kill_sigterm(process_id: u32) {
    let result = unsafe { libc::kill(process_id as i32, libc::SIGTERM) };
    if result != 0 {
        // TODO: handle error
    }
}

#[cfg(unix)]
fn kill_sigkill(process_id: u32) {
    let result = unsafe { libc::kill(process_id as i32, libc::SIGKILL) };
    if result != 0 {
        // TODO: handle error
    }
}

#[cfg(windows)]
fn kill_sigterm(process_id: u32) {
    // There's no equivalent to SIGTERM on win32 (so far as I know?), so we just
    // resort directly to the equivalent of SIGKILL.
    kill_sigkill(process_id);
}

#[cfg(windows)]
fn kill_sigkill(process_id: u32) {
    // TODO: handle errors
    let handle = unsafe { winapi::um::processthreadsapi::OpenProcess(winapi::um::winnt::PROCESS_TERMINATE, 0, process_id) };
    unsafe { winapi::um::processthreadsapi::TerminateProcess(handle, 127) };
}
