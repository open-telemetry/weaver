// SPDX-License-Identifier: Apache-2.0

//! Command to list the supported languages

use clap::Parser;
use std::path::PathBuf;

use weaver_logger::Logger;

/// Parameters for the `languages` command
#[derive(Parser)]
pub struct LanguagesParams {
    /// Template root directory
    #[arg(short, long, default_value = "templates")]
    templates: PathBuf,
}

/// List of supported languages
#[no_coverage]
pub fn command_languages(log: impl Logger + Sync + Clone, params: &LanguagesParams) {
    // List all directories in the templates directory
    log.log("List of supported languages:");
    let template_dir = match std::fs::read_dir(&params.templates) {
        Ok(dir) => dir,
        Err(e) => {
            log.error(&format!("Failed to read templates directory: {}", e));
            std::process::exit(1);
        }
    };
    for entry in template_dir {
        if let Ok(entry) = entry {
            if entry.file_type().is_ok() {
                log.indent(1);
                log.log(&format!(
                    "- {}",
                    entry.file_name().to_str().expect("Invalid file name")
                ));
            }
        } else {
            log.error("Failed to read template directory entry");
            std::process::exit(1);
        }
    }
}
