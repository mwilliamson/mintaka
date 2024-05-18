use wezterm_term::{Screen, VisibleRowIndex};

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

fn status_by_last_matching_line(screen: &Screen, line_to_status: impl Fn(&str) -> Option<ProcessStatus>) -> Option<ProcessStatus> {
    let lines = screen.lines_in_phys_range(screen.phys_range(&(0..VisibleRowIndex::MAX)));
    // TODO: ignore rows we've already processed
    for line in lines.iter().rev() {
        let line_str = line.as_str();
        if let Some(line_status) = line_to_status(&line_str) {
            return Some(line_status);
        }
    }

    None
}
