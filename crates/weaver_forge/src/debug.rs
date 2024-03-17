// SPDX-License-Identifier: Apache-2.0

//! Utility functions to help with debugging.

use crate::error::Error;
use crate::error::Error::{CompoundError, TemplateEvaluationFailed};
use indexmap::IndexMap;
use weaver_logger::Logger;

/// Return a nice summary of the error.
pub(crate) fn error_summary(error: minijinja::Error) -> String {
    format!("{:#}", error)
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
/// * `logger` - The logger to use for logging.
/// * `error` - The error to print.
pub fn print_dedup_errors(logger: impl Logger + Sync + Clone, error: Error) {
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
            1 => err.error.to_string(),
            2 => format!("{}\n\nFound 1 similar error", err.error),
            _ => format!(
                "{}\n\nFound {} similar errors",
                err.error,
                err.occurrences - 1
            ),
        };
        logger.error(&output);
    });
}
