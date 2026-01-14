// SPDX-License-Identifier: Apache-2.0

//! Embedded UI assets for the serve command.

use include_dir::{include_dir, Dir};

/// Embedded UI distribution files.
/// These are built from the ui-react/ directory using `npm run build`.
pub(crate) static UI_DIST: Dir<'_> = include_dir!("ui-react/dist");
