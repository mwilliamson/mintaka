use regex::Regex;

use crate::processes::ProcessStatus;

pub(crate) struct ProcessStatusAnalyzer {
    pub(crate) success_regex: Option<Regex>,
    pub(crate) error_regex: Option<Regex>,
}

impl ProcessStatusAnalyzer {
    pub(crate) fn analyze_line(&self, last_line: &str) -> Option<ProcessStatus> {
        if last_line.trim().is_empty() {
            return None;
        }

        if let Some(error_regex) = &self.error_regex {
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

        if let Some(success_regex) = &self.success_regex {
            if success_regex.is_match(last_line) {
                return Some(ProcessStatus::Success);
            }
        }

        Some(ProcessStatus::Running)
    }
}
