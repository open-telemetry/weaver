// SPDX-License-Identifier: Apache-2.0

//!

use std::error::Error;

pub struct CompoundError {
    errors: Vec<dyn Error>
}

pub trait WeaverError<E: Error> {
    /// Handles a list of errors and returns a compound error if the list is not
    /// empty or () if the list is empty.
    fn handle_errors(errors: Vec<E>) -> E;
    // fn compound_error(errors: Vec<E>) -> E;
}

impl <E: Error> WeaverError<E> for Vec<E> {
    /// Handles a list of errors and returns a compound error if the list is not
    /// empty or () if the list is empty.
    fn handle_errors(errors: Vec<E>) -> E {
        if errors.is_empty() {
            Ok(())
        } else {
            Err(Self::compound_error(errors))
        }
    }

    /// Creates a compound error from a list of errors.
    /// Note: All compound errors are flattened.
    // #[must_use]
    // fn compound_error(errors: Vec<Self>) -> Self {
    //     CompoundError(
    //         errors
    //             .into_iter()
    //             .flat_map(|e| match e {
    //                 CompoundError(errors) => errors,
    //                 e => vec![e],
    //             })
    //             .collect(),
    //     )
    // }
}
