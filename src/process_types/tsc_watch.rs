use regex::Regex;
use wezterm_term::Screen;

use crate::processes::ProcessStatus;

lazy_static::lazy_static! {
    static ref STATUS_REGEX: Regex = Regex::new(" Found ([0-9]+) error[s]?\\. Watching for file changes\\.").unwrap();
}

pub(super) fn status(screen: &Screen) -> Option<ProcessStatus> {
    super::status_by_last_matching_line(screen, |line_str| {
        STATUS_REGEX.captures(&line_str).and_then(|captures| {
            let error_count: u64 = captures.get(1).unwrap().as_str().parse().unwrap();
            return if error_count == 0 {
                Some(ProcessStatus::Ok)
            } else {
                Some(ProcessStatus::Errors { error_count })
            };
        })
    })
}
