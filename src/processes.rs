use std::sync::{Arc, Mutex};

use multimap::MultiMap;
use portable_pty::{ChildKiller, ExitStatus, PtyPair, PtySize, PtySystem};
use termwiz::{
    escape::{parser::Parser, Esc, EscCode},
    terminal::TerminalWaker,
};
use wezterm_term::{TerminalSize, VisibleRowIndex};

use crate::{config::ProcessConfig, process_statuses::ProcessStatusAnalyzer};

type SharedPtySystem = Arc<Box<dyn PtySystem + Send>>;

pub(crate) struct Processes {
    autofocus: bool,

    pty_system: SharedPtySystem,

    pty_size: portable_pty::PtySize,

    processes: Vec<Process>,

    pub(crate) focused_process_index: usize,

    on_change: TerminalWaker,

    after: MultiMap<String, usize>,
}

impl Processes {
    pub(crate) fn new(on_change: TerminalWaker) -> Self {
        let pty_system = Arc::new(portable_pty::native_pty_system());
        let pty_size = portable_pty::PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        Self {
            autofocus: true,
            pty_system,
            pty_size,
            processes: Vec::new(),
            focused_process_index: 0,
            on_change,
            after: MultiMap::new(),
        }
    }

    pub(crate) fn disable_autofocus(&mut self) {
        self.autofocus = false;
    }

    pub(crate) fn toggle_autofocus(&mut self) {
        self.autofocus = !self.autofocus;
    }

    pub(crate) fn autofocus(&self) -> bool {
        self.autofocus
    }

    pub(crate) fn start_process(
        &mut self,
        process_config: ProcessConfig,
    ) -> Result<(), ProcessError> {
        if let Some(after) = &process_config.after {
            self.after.insert(after.to_owned(), self.processes.len());
        }

        let mut process = Process::new(
            process_config,
            Arc::clone(&self.pty_system),
            self.pty_size,
            self.on_change.clone(),
        );

        process.do_work()?;

        self.processes.push(process);

        Ok(())
    }

    pub(crate) fn do_work(&mut self) -> Result<(), ProcessError> {
        self.handle_status_updates();

        for process in &mut self.processes {
            process.do_work()?;
        }

        if self.autofocus {
            self.focused_process_index = self
                .processes
                .iter()
                .enumerate()
                .find(|(_process_index, process)| process.status().is_failure())
                .map(|(process_index, _process)| process_index)
                .unwrap_or(self.focused_process_index);
        }

        Ok(())
    }

    fn handle_status_updates(&mut self) {
        let mut new_statuses = Vec::new();

        for process in &mut self.processes {
            let new_status = process.handle_status_updates();
            if let Some(new_status) = new_status {
                new_statuses.push((process.name().to_string(), new_status));
            }
        }

        for (before_process_name, before_new_status) in new_statuses {
            if let Some(after_process_indexes) = self.after.get_vec_mut(&before_process_name) {
                if before_new_status.is_success() {
                    for process_index in after_process_indexes {
                        self.processes[*process_index].restart()
                    }
                } else {
                    for process_index in after_process_indexes {
                        self.processes[*process_index].mark_waiting_for_upstream()
                    }
                }
            }
        }
    }

    pub(crate) fn processes(&self) -> &[Process] {
        &self.processes
    }

    pub(crate) fn lines(&self) -> Vec<wezterm_term::Line> {
        self.processes[self.focused_process_index].lines()
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
        let pty_size = PtySize {
            cols: size.0 as u16,
            rows: size.1 as u16,
            ..self.pty_size
        };

        if self.pty_size != pty_size {
            self.pty_size = pty_size;

            for process in &mut self.processes {
                process.resize(self.pty_size);
            }
        }
    }

    pub(crate) fn restart_focused(&mut self) {
        self.processes[self.focused_process_index].restart();
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ProcessStatus {
    /// The process has not been started.
    NotStarted,

    /// The process will run once an upstream process reaches a success state.
    WaitingForUpstream,

    /// The process is running and has not reached a success or error state.
    Running,

    /// The process is running and has reached a success state.
    Success,

    /// The process is running and has reached an error state.
    Errors { error_count: Option<u64> },

    /// The process has exited.
    Exited { exit_code: u32 },
}

impl ProcessStatus {
    fn is_failure(&self) -> bool {
        match self {
            ProcessStatus::NotStarted => false,
            ProcessStatus::WaitingForUpstream => false,
            ProcessStatus::Running => false,
            ProcessStatus::Success => false,
            ProcessStatus::Errors { .. } => true,
            ProcessStatus::Exited { exit_code } => *exit_code != 0,
        }
    }

    fn is_success(&self) -> bool {
        match self {
            ProcessStatus::NotStarted => false,
            ProcessStatus::WaitingForUpstream => false,
            ProcessStatus::Running => false,
            ProcessStatus::Success => true,
            ProcessStatus::Errors { .. } => false,
            ProcessStatus::Exited { exit_code } => *exit_code == 0,
        }
    }
}

enum ProcessInstanceState {
    /// This process has not yet been triggered.
    NotStarted,

    /// This process will be triggered once an upstream process reaches a
    /// success state.
    WaitingForUpstream,

    /// This process should be restarted.
    PendingRestart,

    /// This process has a running instance.
    Running {
        instance: ProcessInstance,
        status: ProcessStatus,
        status_rx: std::sync::mpsc::Receiver<ProcessStatus>,
    },
}

pub(crate) struct Process {
    name: String,
    process_config: ProcessConfig,
    // TODO: bundle up pty_system and pty_size?
    pty_system: SharedPtySystem,
    pty_size: PtySize,
    instance_state: ProcessInstanceState,
    on_change: TerminalWaker,
}

impl Process {
    fn new(
        process_config: ProcessConfig,
        pty_system: SharedPtySystem,
        pty_size: PtySize,
        on_change: TerminalWaker,
    ) -> Self {
        let name = process_config
            .name
            .clone()
            .unwrap_or_else(|| process_config.command.join(" "));

        let instance_state = if process_config.autostart() {
            ProcessInstanceState::PendingRestart
        } else {
            ProcessInstanceState::NotStarted
        };

        Self {
            name,
            process_config,
            pty_system,
            pty_size,
            instance_state,
            on_change,
        }
    }

    fn start(&mut self) -> Result<(), ProcessError> {
        let pty_pair = self.pty_system.openpty(self.pty_size).unwrap();

        let (status_tx, status_rx) = std::sync::mpsc::channel();

        let instance = ProcessInstance::start(
            &self.process_config,
            pty_pair,
            self.on_change.clone(),
            status_tx,
        )?;

        self.instance_state = ProcessInstanceState::Running {
            instance,
            status: ProcessStatus::Running,
            status_rx,
        };

        Ok(())
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    fn restart(&mut self) {
        self.kill(ProcessInstanceState::PendingRestart);
    }

    fn mark_waiting_for_upstream(&mut self) {
        self.kill(ProcessInstanceState::WaitingForUpstream);
    }

    fn kill(&mut self, new_process_instance_state: ProcessInstanceState) {
        let previous_instance_state =
            std::mem::replace(&mut self.instance_state, new_process_instance_state);
        if let ProcessInstanceState::Running { mut instance, .. } = previous_instance_state {
            instance.kill();
        }
    }

    fn handle_status_updates(&mut self) -> Option<ProcessStatus> {
        match &mut self.instance_state {
            ProcessInstanceState::NotStarted
            | ProcessInstanceState::WaitingForUpstream
            | ProcessInstanceState::PendingRestart => None,
            ProcessInstanceState::Running {
                status, status_rx, ..
            } => {
                let new_status = status_rx.try_iter().last();

                if let Some(new_status) = new_status {
                    *status = new_status;
                }

                new_status
            }
        }
    }

    fn do_work(&mut self) -> Result<(), ProcessError> {
        if matches!(self.instance_state, ProcessInstanceState::PendingRestart) {
            self.start()?;
        }

        Ok(())
    }

    fn resize(&mut self, pty_size: PtySize) {
        self.pty_size = pty_size;
        if let ProcessInstanceState::Running { instance, .. } = &mut self.instance_state {
            instance.resize(pty_size);
        }
    }

    pub(crate) fn status(&self) -> ProcessStatus {
        match &self.instance_state {
            ProcessInstanceState::NotStarted => ProcessStatus::NotStarted,
            ProcessInstanceState::WaitingForUpstream => ProcessStatus::WaitingForUpstream,
            ProcessInstanceState::PendingRestart => ProcessStatus::Running,
            ProcessInstanceState::Running { status, .. } => *status,
        }
    }

    fn lines(&self) -> Vec<wezterm_term::Line> {
        match &self.instance_state {
            ProcessInstanceState::NotStarted
            | ProcessInstanceState::WaitingForUpstream
            | ProcessInstanceState::PendingRestart => Vec::new(),
            ProcessInstanceState::Running { instance, .. } => instance.lines(),
        }
    }
}

pub(crate) struct ProcessInstance {
    terminal: Arc<Mutex<wezterm_term::Terminal>>,
    pty_master: Box<dyn portable_pty::MasterPty>,
    child_process_killer: Box<dyn ChildKiller + Send + Sync>,
}

impl ProcessInstance {
    fn start(
        process_config: &ProcessConfig,
        pty_pair: PtyPair,
        on_change: TerminalWaker,
        status_tx: std::sync::mpsc::Sender<ProcessStatus>,
    ) -> Result<Self, ProcessError> {
        let pty_command = Self::process_config_to_pty_command(&process_config)?;

        let child_process = pty_pair.slave.spawn_command(pty_command).unwrap();
        let child_process_killer = child_process.clone_killer();
        std::mem::drop(pty_pair.slave);

        let pty_size = pty_pair.master.get_size().unwrap();
        let child_process_writer = pty_pair.master.take_writer().unwrap();
        let terminal = Arc::new(Mutex::new(Self::create_process_terminal(
            child_process_writer,
            pty_size,
        )));

        let child_process_reader = pty_pair.master.try_clone_reader().unwrap();
        Self::spawn_process_reader(
            process_config.process_status_analyzer(),
            child_process,
            child_process_reader,
            Arc::clone(&terminal),
            on_change,
            status_tx,
        );

        Ok(Self {
            terminal,
            pty_master: pty_pair.master,
            child_process_killer,
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
    ) {
        std::thread::spawn(move || {
            let mut bytes = vec![0; 256];
            let mut parser = Parser::new();
            // TODO: Perhaps rather than separately storing the last line, track
            // whether the screen has been cleared and use stable lines in the
            // terminal screen?
            let mut last_line = String::new();

            loop {
                let bytes_read = reader.read(&mut bytes).unwrap();
                if bytes_read == 0 {
                    // TODO: handle failure to get exit code properly
                    let exit_code = child_process
                        .wait()
                        .unwrap_or(ExitStatus::with_exit_code(1));

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
                            if let Some(new_status) =
                                process_status_analyzer.analyze_line(&last_line)
                            {
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

    fn kill(&mut self) {
        // Failures to kill are (hopefully) because the process has already
        // stopped. We could check the status of the child process, but this
        // may lead to a race condition.
        //
        // We could check the error we get back, but since the kind is
        // `Uncategorized`, we'd need to check the message which feels fragile.
        let _ = self.child_process_killer.kill();
    }

    fn resize(&mut self, pty_size: PtySize) {
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

    fn lines(&self) -> Vec<wezterm_term::Line> {
        let terminal = self.terminal.lock().unwrap();
        terminal
            .screen()
            .lines_in_phys_range(terminal.screen().phys_range(&(0..VisibleRowIndex::MAX)))
    }
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
