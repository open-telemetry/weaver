// SPDX-License-Identifier: Apache-2.0

//! The variables an expression is given.

use cel::Context;

use crate::Referenced;

/// Supplies the variables an expression reads.
///
/// Telemetry samples implement this and decide the variable names.
pub trait Bindings {
    /// Binds only the variables named in `referenced`.
    fn bind(&self, referenced: &Referenced, context: &mut Context<'_>);
}
