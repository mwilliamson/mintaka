use wezterm_term::Screen;

use crate::processes::ProcessStatus;

mod tsc_watch;

pub(crate) enum ProcessType {
    TscWatch,
    Unknown,
}

pub(crate) fn status(process_type: &ProcessType, screen: &Screen) -> Option<ProcessStatus> {
    match process_type {
        ProcessType::TscWatch => tsc_watch::status(screen),
        ProcessType::Unknown => None,
    }
}
