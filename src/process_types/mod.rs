use crate::processes::ProcessStatus;

mod tsc_watch;

pub(crate) enum ProcessType {
    TscWatch,
    Unknown,
}

pub(crate) fn status(process_type: &ProcessType, last_line: &str) -> Option<ProcessStatus> {
    match process_type {
        ProcessType::TscWatch => tsc_watch::status(last_line),
        ProcessType::Unknown => None,
    }
}
