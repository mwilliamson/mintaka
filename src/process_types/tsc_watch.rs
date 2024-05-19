use regex::Regex;

use crate::processes::ProcessStatus;

lazy_static::lazy_static! {
    static ref STATUS_REGEX: Regex = Regex::new(" Found ([0-9]+) error[s]?\\. Watching for file changes\\.").unwrap();
}

pub(super) fn status(last_line: &str) -> Option<ProcessStatus> {
    if last_line.trim().is_empty() {
        None
    } else {
        match STATUS_REGEX.captures(last_line) {
            None => Some(ProcessStatus::Running),
            Some(captures) => {
                let error_count: u64 = captures.get(1).unwrap().as_str().parse().unwrap();
                if error_count == 0 {
                    Some(ProcessStatus::Success)
                } else {
                    Some(ProcessStatus::Errors { error_count: Some(error_count) })
                }
            }
        }
    }
}
