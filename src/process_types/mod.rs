use regex::Regex;

use crate::processes::ProcessStatus;

mod tsc_watch;

pub(crate) enum ProcessType {
    TscWatch,
    Unknown { success_regex: Option<Regex>, error_regex: Option<Regex>},
}

pub(crate) fn status(process_type: &ProcessType, last_line: &str) -> Option<ProcessStatus> {
    match process_type {
        ProcessType::TscWatch => tsc_watch::status(last_line),
        ProcessType::Unknown { success_regex, error_regex } => {
            if last_line.trim().is_empty() {
                return None;
            }

            if let Some(error_regex) = error_regex {
                match error_regex.captures(last_line) {
                    None => {},
                    Some(captures) => {
                        let error_count: Option<u64> = captures.get(1).and_then(|capture| capture.as_str().parse().ok());
                        if error_count == Some(0) {
                            return Some(ProcessStatus::Success);
                        } else {
                            return Some(ProcessStatus::Errors { error_count })
                        }
                    }
                }
            }

            if let Some(success_regex) = success_regex {
                if success_regex.is_match(last_line) {
                    return Some(ProcessStatus::Success);
                }
            }

            Some(ProcessStatus::Running)
        },
    }
}
