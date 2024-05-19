use std::{collections::HashSet, sync::{Arc, Mutex}};

use multimap::MultiMap;
use portable_pty::{ChildKiller, ExitStatus, PtyPair, PtySize, PtySystem};
use termwiz::{escape::{parser::Parser, Esc, EscCode}, terminal::TerminalWaker};
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

    success_notifications: Arc<Mutex<HashSet<String>>>,
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
            success_notifications: Arc::new(Mutex::new(HashSet::new())),
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
            Arc::clone(&self.success_notifications),
        );

        process.do_work()?;

        self.processes.push(process);

        Ok(())
    }

    pub(crate) fn do_work(&mut self) -> Result<(), ProcessError> {
        let mut success_notifications_locked = self.success_notifications.lock().unwrap();

        for process_name in success_notifications_locked.drain() {
            if let Some(process_indexes) = self.after.get_vec_mut(&process_name) {
                for process_index in process_indexes {
                    self.processes[*process_index].restart()
                }
            }
        }

        for process in &mut self.processes {
            process.do_work()?;
        }

        if self.autofocus {
            self.focused_process_index = self.processes.iter()
                .enumerate()
                .find(|(_process_index, process)| !process.status().is_ok())
                .map(|(process_index, _process)| process_index)
                .unwrap_or(self.focused_process_index);
        }

        Ok(())
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

    /// The process is running and has not reached a success or error state.
    Running,

    /// The process is running and has reached a success state.
    Success,

    /// The process is running and has reached an error state.
    Errors {
        error_count: Option<u64>,
    },

    /// The process has exited.
    Exited {
        exit_code: u32,
    },
}

impl ProcessStatus {
    pub(crate) fn is_ok(&self) -> bool {
        match self {
            ProcessStatus::NotStarted => true,
            ProcessStatus::Running => true,
            ProcessStatus::Success => true,
            ProcessStatus::Errors { .. } => false,
            ProcessStatus::Exited { exit_code } => *exit_code == 0,
        }
    }
}

pub(crate) struct Process {
    name: String,
    process_config: ProcessConfig,
    // TODO: bundle up pty_system and pty_size?
    pty_system: SharedPtySystem,
    pty_size: PtySize,
    instance: Option<ProcessInstance>,
    pending_restart: bool,
    on_change: TerminalWaker,
    success_notifications: Arc<Mutex<HashSet<String>>>,
}

impl Process {
    fn new(
        process_config: ProcessConfig,
        pty_system: SharedPtySystem,
        pty_size: PtySize,
        on_change: TerminalWaker,
        success_notifications: Arc<Mutex<HashSet<String>>>,
    ) -> Self {
        let name = process_config.name.clone()
            .unwrap_or_else(|| process_config.command.join(" "));

        let pending_restart = process_config.autostart();

        Self {
            name,
            process_config,
            pty_system,
            pty_size,
            instance: None,
            pending_restart,
            on_change,
            success_notifications,
        }
    }

    fn start(
        &mut self,
    ) -> Result<(), ProcessError> {
        let pty_pair = self.pty_system.openpty(self.pty_size).unwrap();

        let instance = ProcessInstance::start(
            &self.process_config,
            pty_pair,
            self.on_change.clone(),
            Arc::clone(&self.success_notifications),
        )?;

        self.instance = Some(instance);

        Ok(())
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    fn restart(&mut self) {
        if let Some(instance) = &mut self.instance {
            instance.kill();
        }
        self.pending_restart = true;
    }

    fn do_work(&mut self) -> Result<(), ProcessError> {
        if self.pending_restart && self.instance_is_stopped() {
            self.start()?;
            self.pending_restart = false;
        }

        Ok(())
    }

    fn instance_is_stopped(&self) -> bool {
        match &self.instance {
            None => true,
            Some(instance) => instance.is_stopped()
        }
    }

    fn resize(&mut self, pty_size: PtySize) {
        self.pty_size = pty_size;
        if let Some(instance) = &mut self.instance {
            instance.resize(pty_size);
        }
    }

    pub(crate) fn status(&self) -> ProcessStatus {
        match &self.instance {
            None => ProcessStatus::NotStarted,
            Some(instance) => instance.status(),
        }

    }

    fn lines(&self) -> Vec<wezterm_term::Line> {
        match &self.instance {
            None => Vec::new(),
            Some(instance) => instance.lines(),
        }
    }
}

pub(crate) struct ProcessInstance {
    status: Arc<Mutex<ProcessStatus>>,
    terminal: Arc<Mutex<wezterm_term::Terminal>>,
    pty_master: Box<dyn portable_pty::MasterPty>,
    child_process_killer: Box<dyn ChildKiller + Send + Sync>,
}

impl ProcessInstance {
    fn start(
        process_config: &ProcessConfig,
        pty_pair: PtyPair,
        on_change: TerminalWaker,
        success_notifications: Arc<Mutex<HashSet<String>>>,
    ) -> Result<Self, ProcessError> {
        let process_name = process_config.name.clone()
            .unwrap_or_else(|| process_config.command.join(" "));

        let pty_command = Self::process_config_to_pty_command(&process_config)?;

        let child_process = pty_pair.slave.spawn_command(pty_command).unwrap();
        let child_process_killer = child_process.clone_killer();
        std::mem::drop(pty_pair.slave);

        let child_process_writer = pty_pair.master.take_writer().unwrap();
        let terminal = Arc::new(Mutex::new(Self::create_process_terminal(child_process_writer)));

        let process_status = Arc::new(Mutex::new(ProcessStatus::Running));

        let child_process_reader = pty_pair.master.try_clone_reader().unwrap();
        Self::spawn_process_reader(
            process_name.clone(),
            process_config.process_status_analyzer(),
            child_process,
            child_process_reader,
            Arc::clone(&process_status),
            Arc::clone(&terminal),
            on_change,
            success_notifications,
        );

        Ok(Self {
            status: process_status,
            terminal,
            pty_master: pty_pair.master,
            child_process_killer,
        })
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
        process_name: String,
        process_status_analyzer: ProcessStatusAnalyzer,
        mut child_process: Box<dyn portable_pty::Child>,
        mut reader: Box<dyn std::io::Read + Send>,
        process_status: Arc<Mutex<ProcessStatus>>,
        terminal: Arc<Mutex<wezterm_term::Terminal>>,
        on_change: TerminalWaker,
        success_notifications: Arc<Mutex<HashSet<String>>>,
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
                    let exit_code = child_process.wait().unwrap_or(ExitStatus::with_exit_code(1));

                    let mut process_status_locked = process_status.lock().unwrap();
                    *process_status_locked = ProcessStatus::Exited { exit_code: exit_code.exit_code() };

                    if exit_code.success() {
                        let mut success_notifications_locked = success_notifications.lock().unwrap();
                        success_notifications_locked.insert(process_name.clone());
                    }

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
                            termwiz::escape::ControlCode::LineFeed |
                            termwiz::escape::ControlCode::CarriageReturn
                        ) |
                        termwiz::escape::Action::Esc(Esc::Code(EscCode::FullReset)) => {
                            if let Some(new_status) = process_status_analyzer.analyze_line(&last_line) {
                                let mut process_status_locked = process_status.lock().unwrap();
                                *process_status_locked = new_status;

                                if matches!(new_status, ProcessStatus::Success) {
                                    let mut success_notifications_locked = success_notifications.lock().unwrap();
                                    success_notifications_locked.insert(process_name.clone());
                                }
                            }

                            last_line.clear();
                        },
                        _ => {},
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

    fn is_stopped(&self) -> bool {
        matches!(self.status(), ProcessStatus::Exited { .. })
    }

    fn status(&self) -> ProcessStatus {
        *self.status.lock().unwrap()
    }

    fn lines(&self) -> Vec<wezterm_term::Line> {
        let terminal = self.terminal.lock().unwrap();
        terminal.screen().lines_in_phys_range(terminal.screen().phys_range(&(0..VisibleRowIndex::MAX)))
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
