use regex::Regex;
use wezterm_term::{Screen, VisibleRowIndex};

use crate::processes::ProcessStatus;

lazy_static::lazy_static! {
    static ref STATUS_REGEX: Regex = Regex::new(" Found ([0-9]+) error[s]?\\. Watching for file changes\\.").unwrap();
}

pub(super) fn status(screen: &Screen) -> Option<ProcessStatus> {
    let rows = screen.lines_in_phys_range(screen.phys_range(&(0..VisibleRowIndex::MAX)));
    // TODO: ignore rows we've already processed
    for row in rows.iter().rev() {
        let row_str = row.as_str();
        match STATUS_REGEX.captures(&row_str) {
            Some(captures) => {
                let error_count: u64 = captures.get(1).unwrap().as_str().parse().unwrap();
                return if error_count == 0 {
                    Some(ProcessStatus::Ok)
                } else {
                    Some(ProcessStatus::Errors { error_count })
                };
            },
            None => {},
        }
    }

    None
}
