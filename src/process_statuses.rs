use regex::Regex;

pub(crate) struct ProcessStatusAnalyzer {
    pub(crate) success_regex: Option<Regex>,
    pub(crate) error_regex: Option<Regex>,
}

/// The result of analysing a line.
pub(crate) enum LineAnalysis {
    /// The line represents neither a success nor an error.
    Running,

    /// The line represents a success.
    Success,

    /// The line represents an error.
    Errors { error_count: Option<u64> },
}

impl ProcessStatusAnalyzer {
    pub(crate) fn analyze_line(&self, last_line: &str) -> Option<LineAnalysis> {
        if last_line.trim().is_empty() {
            return None;
        }

        if let Some(error_regex) = &self.error_regex {
            match error_regex.captures(last_line) {
                None => {}
                Some(captures) => {
                    let error_count: Option<u64> = captures
                        .get(1)
                        .and_then(|capture| capture.as_str().parse().ok());
                    if error_count == Some(0) {
                        return Some(LineAnalysis::Success);
                    } else {
                        return Some(LineAnalysis::Errors { error_count });
                    }
                }
            }
        }

        if let Some(success_regex) = &self.success_regex {
            if success_regex.is_match(last_line) {
                return Some(LineAnalysis::Success);
            }
        }

        Some(LineAnalysis::Running)
    }
}
