use regex::Regex;

use crate::processes::ProcessStatus;

lazy_static::lazy_static! {
    static ref STATUS_REGEX: Regex = Regex::new(" Found ([0-9]+) error[s]?\\. Watching for file changes\\.").unwrap();
}

pub(super) fn status(last_line: &str) -> Option<ProcessStatus> {
    STATUS_REGEX.captures(last_line).and_then(|captures| {
        let error_count: u64 = captures.get(1).unwrap().as_str().parse().unwrap();
        return if error_count == 0 {
            Some(ProcessStatus::Running)
        } else {
            Some(ProcessStatus::Errors { error_count })
        };
    })
}
