use std::{collections::HashMap, sync::Arc};

use errors::ProcessError;
use instances::{ProcessInstance, ProcessInstanceId};
use portable_pty::{PtySize, PtySystem};
pub(crate) use scroll::ScrollDirection;
use snapshots::ProcessSnapshot;
pub(crate) use statuses::ProcessStatus;
use termwiz::{input::KeyEvent, terminal::TerminalWaker};
use wezterm_term::CursorPosition;

use crate::config::ProcessConfig;

mod errors;
mod instances;
mod scroll;
mod snapshots;
mod statuses;

type SharedPtySystem = Arc<Box<dyn PtySystem + Send>>;

#[derive(Clone, Copy)]
pub(crate) enum MintakaMode {
    /// The main mode, allowing focus to be manually or automatically switched
    /// between each process.
    Main,

    /// Any input to Mintaka should be forwarded to the focused process, with
    /// the exception of the input to stop forwarding input.
    ///
    /// TODO: how does a user send the key sequence to leave the process to the
    /// process?
    ForwardInputToFocusedProcess,

    /// Scroll through the history of the process.
    History,
}

pub(crate) struct Processes {
    /// Whether the user has enabled autofocus. Autofocus may be suspended, for
    /// instance while a process is entered.
    autofocus_enabled: bool,

    mode: MintakaMode,

    snapshot: Option<ProcessSnapshot>,

    pty_size: portable_pty::PtySize,

    processes: Vec<Process>,

    pub(crate) focused_process_index: usize,

    after: Vec<DownstreamProcesses>,
}

impl Processes {
    pub(crate) fn new(on_change: TerminalWaker, process_configs: Vec<ProcessConfig>) -> Self {
        let pty_system = Arc::new(portable_pty::native_pty_system());
        let pty_size = portable_pty::PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        };

        let mut processes = Vec::new();
        let mut after: HashMap<_, _> = process_configs
            .iter()
            .enumerate()
            .map(|(process_index, process_config)| {
                (
                    process_config.name(),
                    DownstreamProcesses {
                        upstream_process_index: process_index,
                        last_upstream_status: ProcessStatus::NotStarted,
                        downstream_process_indexes: Vec::new(),
                    },
                )
            })
            .collect();

        for process_config in process_configs {
            if let Some(upstream_process_name) = &process_config.after {
                // TODO: handle incorrect upstream process name.
                if let Some(downstream_processes) = after.get_mut(upstream_process_name) {
                    let process_index = processes.len();
                    downstream_processes
                        .downstream_process_indexes
                        .push(process_index);
                }
            }

            let process = Process::new(
                process_config,
                Arc::clone(&pty_system),
                pty_size,
                on_change.clone(),
            );

            processes.push(process);
        }

        let mut after: Vec<_> = after.into_values().collect();
        after.sort_by_key(|downstream_processes| downstream_processes.upstream_process_index);

        Self {
            autofocus_enabled: true,
            mode: MintakaMode::Main,
            snapshot: None,
            pty_size,
            processes,
            focused_process_index: 0,
            after,
        }
    }

    pub(crate) fn disable_autofocus(&mut self) {
        self.autofocus_enabled = false;
    }

    pub(crate) fn toggle_autofocus(&mut self) {
        self.autofocus_enabled = !self.autofocus_enabled;
    }

    pub(crate) fn autofocus_enabled(&self) -> bool {
        self.autofocus_enabled
    }

    pub(crate) fn forward_input_to_focused_process(&mut self) {
        self.mode = MintakaMode::ForwardInputToFocusedProcess;
    }

    pub(crate) fn enter_main_mode(&mut self) {
        self.mode = MintakaMode::Main;
    }

    pub(crate) fn scroll(&mut self, direction: ScrollDirection) {
        self.mode = MintakaMode::History;

        if self.snapshot.is_none() {
            self.snapshot = Some(self.processes[self.focused_process_index].snapshot());
        }

        if let Some(snapshot) = &mut self.snapshot {
            snapshot.scroll(direction);
        }
    }

    pub(crate) fn leave_history(&mut self) {
        self.mode = MintakaMode::Main;
        self.snapshot = None;
    }

    pub(crate) fn mode(&self) -> MintakaMode {
        self.mode
    }

    /// Whether autofocus should currently be used.
    fn should_autofocus(&self) -> bool {
        match self.mode {
            MintakaMode::Main => self.autofocus_enabled,
            MintakaMode::ForwardInputToFocusedProcess | MintakaMode::History => false,
        }
    }

    pub(crate) fn stop_all(&mut self) {
        // TODO: prevent processes from automatically restarting after stopping.
        for process in &mut self.processes {
            process.stop();
        }
    }

    /// Whether or not all processes have stopped.
    pub(crate) fn is_stopped(&self) -> bool {
        self.processes.iter().all(|process| process.is_stopped())
    }

    pub(crate) fn do_work(&mut self) -> Result<(), ProcessError> {
        self.handle_status_updates();

        for process in &mut self.processes {
            process.do_work()?;
        }

        if self.should_autofocus() {
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
        for process in &mut self.processes {
            process.synchronize_status();
        }

        for downstream_processes in &mut self.after {
            let upstream_process = &self.processes[downstream_processes.upstream_process_index];
            let downstream_action =
                downstream_processes.update_upstream_status(upstream_process.status());

            if let Some(downstream_action) = downstream_action {
                for downstream_process_index in &downstream_processes.downstream_process_indexes {
                    let downstream_process = &mut self.processes[*downstream_process_index];

                    match downstream_action {
                        DownstreamAction::Restart => downstream_process.restart(),
                        DownstreamAction::WaitForUpstream => {
                            downstream_process.mark_waiting_for_upstream()
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn processes(&self) -> &[Process] {
        &self.processes
    }

    pub(crate) fn screen_contents(&self) -> ScreenContents {
        match self.mode {
            MintakaMode::Main => {
                let process = &self.processes[self.focused_process_index];
                process.screen_contents().without_cursor()
            }
            MintakaMode::ForwardInputToFocusedProcess => {
                let process = &self.processes[self.focused_process_index];
                process.screen_contents()
            }
            MintakaMode::History => {
                let lines = if let Some(snapshot) = &self.snapshot {
                    snapshot.lines()
                } else {
                    // TODO: warning? Or make this state unrepresentable?
                    Vec::new()
                };
                ScreenContents::Terminal {
                    lines,
                    cursor_position: None,
                }
            }
        }
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

    pub(crate) fn send_input(&mut self, input: KeyEvent) {
        self.processes[self.focused_process_index].send_input(input)
    }
}

/// Track a set of downstream processes.
struct DownstreamProcesses {
    /// The index of the upstream process in [`Processes::processes`].
    upstream_process_index: usize,

    /// The index of the downstream processes in [`Processes::processes`].
    downstream_process_indexes: Vec<usize>,

    /// The last upstream status that was acted upon.
    last_upstream_status: ProcessStatus,
}

impl DownstreamProcesses {
    /// Update the status of the upstream process, returning the action that
    /// should be taken on the downstream processes, if any.
    fn update_upstream_status(
        &mut self,
        upstream_status: ProcessStatus,
    ) -> Option<DownstreamAction> {
        let downstream_action = if self.last_upstream_status == upstream_status {
            None
        } else if upstream_status.is_success() {
            Some(DownstreamAction::Restart)
        } else {
            Some(DownstreamAction::WaitForUpstream)
        };

        self.last_upstream_status = upstream_status;

        downstream_action
    }
}

/// The action to take on downstream processes following a change in an upstream
/// process's status.
enum DownstreamAction {
    /// Restart all downstream processes.
    Restart,

    /// Stop all downstream processes, and wait for the upstream process to
    /// succeed.
    WaitForUpstream,
}

enum ProcessInstanceState {
    /// This process has not yet been triggered.
    NotStarted,

    /// This process has been stopped, and will not be automatically started.
    Stopped,

    /// This process will be triggered once an upstream process reaches a
    /// success state.
    WaitingForUpstream,

    /// This process should be restarted.
    PendingRestart,

    /// Failed to start a process instance.
    FailedToStart(ProcessError),

    /// This process has a running instance.
    Running {
        instance: ProcessInstance,
        status: ProcessStatus,
        status_rx: std::sync::mpsc::Receiver<ProcessStatus>,
    },

    /// This process has a running instance that is being terminated.
    Terminating {
        // TODO: remove duplication with Running
        instance: ProcessInstance,
        status: ProcessStatus,
        status_rx: std::sync::mpsc::Receiver<ProcessStatus>,

        /// The state that the process should have once it has been terminated.
        state_on_termination: Box<ProcessInstanceState>,
    },
}

impl ProcessInstanceState {
    /// Whether or not the process is stopped, meaning it should not be started
    /// automatically.
    fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped)
    }

    /// Whether or not the process is stopping.
    fn is_stopping(&self) -> bool {
        if let Self::Terminating {
            state_on_termination,
            ..
        } = self
        {
            matches!(state_on_termination.as_ref(), Self::Stopped)
        } else {
            false
        }
    }

    /// Convert the process instance state to a process status.
    fn to_status(&self) -> ProcessStatus {
        match self {
            ProcessInstanceState::NotStarted => ProcessStatus::NotStarted,
            ProcessInstanceState::Stopped => ProcessStatus::Stopped,
            ProcessInstanceState::WaitingForUpstream => ProcessStatus::WaitingForUpstream,
            ProcessInstanceState::PendingRestart => ProcessStatus::Running,
            ProcessInstanceState::FailedToStart(_) => ProcessStatus::FailedToStart,
            ProcessInstanceState::Running { status, .. } => *status,
            ProcessInstanceState::Terminating {
                state_on_termination,
                ..
            } => match state_on_termination.as_ref() {
                ProcessInstanceState::PendingRestart => ProcessStatus::Restarting,
                ProcessInstanceState::Stopped => ProcessStatus::Stopping,
                _ => ProcessStatus::Terminating,
            },
        }
    }
}

pub(crate) struct Process {
    name: String,
    process_config: ProcessConfig,
    // TODO: bundle up pty_system and pty_size?
    pty_system: SharedPtySystem,
    pty_size: PtySize,
    instance_state: ProcessInstanceState,
    on_change: TerminalWaker,
    next_process_instance_id: ProcessInstanceId,
}

impl Process {
    fn new(
        process_config: ProcessConfig,
        pty_system: SharedPtySystem,
        pty_size: PtySize,
        on_change: TerminalWaker,
    ) -> Self {
        let name = process_config.name();

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
            next_process_instance_id: ProcessInstanceId::new(),
        }
    }

    fn start(&mut self) {
        let pty_pair = self.pty_system.openpty(self.pty_size).unwrap();

        let (status_tx, status_rx) = std::sync::mpsc::channel();

        let start_result = ProcessInstance::start(
            &self.process_config,
            pty_pair,
            self.on_change.clone(),
            status_tx,
            self.next_process_instance_id.increment(),
        );

        self.instance_state = match start_result {
            Ok(instance) => ProcessInstanceState::Running {
                instance,
                status: ProcessStatus::Running,
                status_rx,
            },
            Err(error) => ProcessInstanceState::FailedToStart(error),
        };
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    fn stop(&mut self) {
        self.kill(ProcessInstanceState::Stopped);
    }

    /// Whether or not the process has stopped.
    fn is_stopped(&self) -> bool {
        self.instance_state.is_stopped()
    }

    /// Whether or not the process is stopping.
    fn is_stopping(&self) -> bool {
        self.instance_state.is_stopping()
    }

    fn restart(&mut self) {
        self.kill(ProcessInstanceState::PendingRestart);
    }

    fn mark_waiting_for_upstream(&mut self) {
        self.kill(ProcessInstanceState::WaitingForUpstream);
    }

    fn kill(&mut self, state_on_termination: ProcessInstanceState) {
        if self.is_stopped() || self.is_stopping() {
            return;
        }

        let previous_instance_state =
            std::mem::replace(&mut self.instance_state, ProcessInstanceState::NotStarted);

        if let ProcessInstanceState::Running {
            mut instance,
            status,
            status_rx,
        }
        | ProcessInstanceState::Terminating {
            mut instance,
            status,
            status_rx,
            ..
        } = previous_instance_state
        {
            instance.kill();

            self.instance_state = ProcessInstanceState::Terminating {
                instance,
                status,
                status_rx,
                state_on_termination: Box::new(state_on_termination),
            };
        } else {
            self.instance_state = state_on_termination;
        }
    }

    /// Synchronize the status of [`Process`] with the actual process.
    fn synchronize_status(&mut self) {
        match &mut self.instance_state {
            ProcessInstanceState::NotStarted
            | ProcessInstanceState::Stopped
            | ProcessInstanceState::WaitingForUpstream
            | ProcessInstanceState::PendingRestart
            | ProcessInstanceState::FailedToStart(_) => {}
            ProcessInstanceState::Running {
                status, status_rx, ..
            }
            | ProcessInstanceState::Terminating {
                status, status_rx, ..
            } => {
                let new_status = status_rx.try_iter().last();

                if let Some(new_status) = new_status {
                    *status = new_status;
                }
            }
        }

        match &mut self.instance_state {
            ProcessInstanceState::Terminating {
                status: ProcessStatus::Exited { .. },
                state_on_termination,
                ..
            } => {
                self.instance_state =
                    std::mem::replace(state_on_termination, ProcessInstanceState::NotStarted);
            }
            _ => {}
        }
    }

    fn do_work(&mut self) -> Result<(), ProcessError> {
        if matches!(self.instance_state, ProcessInstanceState::PendingRestart) {
            self.start();
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
        self.instance_state.to_status()
    }

    fn screen_contents(&self) -> ScreenContents {
        if let ProcessInstanceState::FailedToStart(error) = &self.instance_state {
            ScreenContents::Error(format!("{:?}", error))
        } else {
            let lines = self.lines();
            let cursor_position = self.cursor_position();
            ScreenContents::Terminal {
                lines,
                cursor_position,
            }
        }
    }

    fn lines(&self) -> Vec<wezterm_term::Line> {
        match &self.instance_state {
            ProcessInstanceState::NotStarted
            | ProcessInstanceState::Stopped
            | ProcessInstanceState::WaitingForUpstream
            | ProcessInstanceState::PendingRestart
            | ProcessInstanceState::FailedToStart(_) => Vec::new(),
            ProcessInstanceState::Running { instance, .. }
            | ProcessInstanceState::Terminating { instance, .. } => instance.lines(),
        }
    }

    fn cursor_position(&self) -> Option<CursorPosition> {
        if !self.status().is_running() {
            return None;
        }

        if let Some(instance) = self.instance() {
            instance.cursor_position()
        } else {
            None
        }
    }

    fn send_input(&self, input: KeyEvent) {
        if let Some(instance) = self.instance() {
            instance.send_input(input);
        }
    }

    fn snapshot(&self) -> ProcessSnapshot {
        if let Some(instance) = self.instance() {
            instance.snapshot()
        } else {
            ProcessSnapshot::empty()
        }
    }

    fn instance(&self) -> Option<&ProcessInstance> {
        match &self.instance_state {
            ProcessInstanceState::NotStarted
            | ProcessInstanceState::Stopped
            | ProcessInstanceState::WaitingForUpstream
            | ProcessInstanceState::PendingRestart
            | ProcessInstanceState::FailedToStart(_) => None,
            ProcessInstanceState::Running { instance, .. }
            | ProcessInstanceState::Terminating { instance, .. } => Some(instance),
        }
    }
}

pub(crate) enum ScreenContents {
    Error(String),
    Terminal {
        lines: Vec<wezterm_term::Line>,
        cursor_position: Option<CursorPosition>,
    },
}

impl ScreenContents {
    fn without_cursor(mut self) -> Self {
        match &mut self {
            ScreenContents::Error(_) => {}
            ScreenContents::Terminal {
                cursor_position, ..
            } => {
                *cursor_position = None;
            }
        }

        self
    }
}
