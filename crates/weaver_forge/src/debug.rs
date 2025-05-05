// SPDX-License-Identifier: Apache-2.0

//! Utility functions to help with debugging.

use crate::error::Error::{CompoundError, TemplateEvaluationFailed};
use indexmap::IndexMap;
use log::error;
use std::error::Error;

/// Return a nice summary of the error including the chain of causes.
/// Only the last error in the chain is displayed with a full stack trace.
pub(crate) fn error_summary(error: minijinja::Error) -> String {
    let mut errors = Vec::new();
    let mut curr_error: &dyn Error = &error;

    errors.push(curr_error);

    while let Some(e) = curr_error.source() {
        errors.push(e);
        curr_error = e;
    }

    let mut error_msg = String::new();
    for (i, e) in errors.iter().enumerate() {
        if i == errors.len() - 1 {
            // Display the last error with all the referenced variables
            error_msg.push_str(&format!("{:#}\n", e));
        } else {
            error_msg.push_str(&format!("{}\nCaused by:\n", e));
        }
    }
    error_msg
}

/// Print deduplicated errors.
///
/// This function prints the error message and the number of occurrences of
/// each error. If an error occurs only once, the error message is printed
/// as is. If an error occurs more than once, the error message is printed
/// once and the number of occurrences is printed as "and n more similar
/// errors".
///
/// The order of the errors is preserved.
///
/// # Arguments
///
/// * `error` - The error to print.
pub fn print_dedup_errors(error: crate::error::Error) {
    struct DedupError {
        pub error: String,
        pub occurrences: usize,
    }

    let mut dedup_errs = IndexMap::new();
    match error {
        CompoundError(errs) => {
            for err in errs {
                match err.clone() {
                    TemplateEvaluationFailed {
                        error_id, error, ..
                    } => {
                        _ = dedup_errs
                            .entry(error_id)
                            .and_modify(|e: &mut DedupError| e.occurrences += 1)
                            .or_insert(DedupError {
                                error,
                                occurrences: 1,
                            });
                    }
                    _ => {
                        _ = dedup_errs
                            .entry(err.to_string())
                            .and_modify(|e: &mut DedupError| e.occurrences += 1)
                            .or_insert(DedupError {
                                error: err.to_string(),
                                occurrences: 1,
                            });
                    }
                }
            }
        }
        _ => {
            _ = dedup_errs
                .entry(error.to_string())
                .and_modify(|e| e.occurrences += 1)
                .or_insert(DedupError {
                    error: error.to_string(),
                    occurrences: 1,
                });
        }
    }
    dedup_errs.iter().for_each(|(_, err)| {
        let output = match err.occurrences {
            1 => err.error.clone(),
            2 => format!("{}\n\nFound 1 similar error", err.error),
            _ => format!(
                "{}\n\nFound {} similar errors",
                err.error,
                err.occurrences - 1
            ),
        };
        error!("{}", &output);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error::TargetNotSupported;

    #[test]
    fn test_print_dedup_errors() {
        let test_log = weaver_common::TestLog::new();

        let error = CompoundError(vec![
            TargetNotSupported {
                // <-- These 3 errors are deduplicated
                root_path: "target".to_owned(),
                target: "target".to_owned(),
                error: "error".to_owned(),
            },
            TargetNotSupported {
                root_path: "target".to_owned(),
                target: "target".to_owned(),
                error: "error".to_owned(),
            },
            TargetNotSupported {
                root_path: "target".to_owned(),
                target: "target".to_owned(),
                error: "error".to_owned(),
            },
            TargetNotSupported {
                // <-- This error is not deduplicated
                root_path: "target".to_owned(),
                target: "other_target".to_owned(),
                error: "error".to_owned(),
            },
        ]);
        print_dedup_errors(error);

        // Check the error count using the TestLog reference
        assert_eq!(test_log.error_count(), 2);
    }
}
