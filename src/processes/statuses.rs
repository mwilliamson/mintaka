use super::ProcessInstanceId;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum ProcessStatus {
    /// The process has not been started.
    NotStarted,

    /// This process has been stopped, and will not be automatically started.
    Stopped,

    /// The process will run once an upstream process reaches a success state.
    WaitingForUpstream,

    /// Failed to start a process instance.
    FailedToStart,

    /// The process is running and has not reached a success or error state.
    Running,

    /// The process is running and has reached a success state.
    Success(SuccessId),

    /// The process is running and has reached an error state.
    Errors { error_count: Option<u64> },

    /// The process has exited.
    Exited { exit_code: u32 },
}

/// The ID of a success.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct SuccessId {
    /// The ID of the process instance.
    process_instance_id: ProcessInstanceId,

    /// The index of the success within the process instance.
    success_index: u32,
}

impl SuccessId {
    /// Create a new success ID for a process instance.
    pub(super) fn new(process_instance_id: ProcessInstanceId) -> Self {
        Self {
            process_instance_id,
            success_index: 0,
        }
    }

    /// Increment the ID, returning the value before the increment.
    pub(super) fn increment(&mut self) -> Self {
        let previous = *self;
        self.success_index += 1;
        previous
    }
}

impl ProcessStatus {
    pub(super) fn is_failure(&self) -> bool {
        match self {
            ProcessStatus::NotStarted => false,
            ProcessStatus::Stopped => false,
            ProcessStatus::WaitingForUpstream => false,
            ProcessStatus::FailedToStart => true,
            ProcessStatus::Running => false,
            ProcessStatus::Success(_) => false,
            ProcessStatus::Errors { .. } => true,
            ProcessStatus::Exited { exit_code } => *exit_code != 0,
        }
    }

    pub(super) fn is_success(&self) -> bool {
        match self {
            ProcessStatus::NotStarted => false,
            ProcessStatus::Stopped => false,
            ProcessStatus::WaitingForUpstream => false,
            ProcessStatus::FailedToStart => false,
            ProcessStatus::Running => false,
            ProcessStatus::Success(_) => true,
            ProcessStatus::Errors { .. } => false,
            ProcessStatus::Exited { exit_code } => *exit_code == 0,
        }
    }

    pub(super) fn is_running(&self) -> bool {
        match self {
            ProcessStatus::NotStarted => false,
            ProcessStatus::Stopped => false,
            ProcessStatus::WaitingForUpstream => false,
            ProcessStatus::FailedToStart => false,
            ProcessStatus::Running => true,
            ProcessStatus::Success(_) => true,
            ProcessStatus::Errors { .. } => true,
            ProcessStatus::Exited { .. } => false,
        }
    }
}
