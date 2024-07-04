// SPDX-License-Identifier: Apache-2.0

//! Search a semantic convention registry.

use clap::Args;
use miette::Diagnostic;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_resolved_schema::ResolvedTelemetrySchema;
use weaver_semconv::registry::SemConvRegistry;

use crate::{
    registry::RegistryArgs,
    util::{load_semconv_specs, resolve_semconv_specs, semconv_registry_path_from},
    DiagnosticArgs, ExitDirectives,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::{CrosstermBackend, Stylize, Terminal},
    widgets::Paragraph,
    Frame,
};
use std::io::stdout;

/// Parameters for the `registry search` sub-command
#[derive(Debug, Args)]
pub struct RegistrySearchArgs {
    /// Parameters to specify the semantic convention registry
    #[command(flatten)]
    registry: RegistryArgs,

    /// Flag to indicate if lineage information should be included in the
    /// resolved schema (not yet implemented)
    #[arg(long, default_value = "false")]
    lineage: bool,

    /// Parameters to specify the diagnostic format.
    #[command(flatten)]
    pub diagnostic: DiagnosticArgs,
}

#[derive(thiserror::Error, Debug, serde::Serialize, Diagnostic)]
enum Error {
    #[error("{0}")]
    StdIoError(String),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::StdIoError(e.to_string())
    }
}

// Our search application state
struct SearchApp<'a> {
    schema: &'a ResolvedTelemetrySchema,
}

impl<'a> SearchApp<'a> {
    fn new(schema: &'a ResolvedTelemetrySchema) -> SearchApp<'a> {
        SearchApp { schema }
    }

    fn render(&self, frame: &mut Frame<'_>) {
        let area = frame.size();
        frame.render_widget(
            Paragraph::new(format!(
                "Hello Search {0}! (press 'q' to quit)",
                self.schema.schema_url
            ))
            .white()
            .on_blue(),
            area,
        );
    }
    // Returns true when it's time to quit.
    fn process(&mut self, event: Event) -> Result<bool, Error> {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

fn run_ui(schema: &ResolvedTelemetrySchema) -> Result<(), Error> {
    let mut app = SearchApp::new(schema);
    let _ = stdout().execute(EnterAlternateScreen)?;
    enable_raw_mode()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    terminal.clear()?;

    // main loop
    loop {
        let _ = terminal.draw(|frame| app.render(frame))?;
        if event::poll(std::time::Duration::from_millis(16))? {
            if app.process(event::read()?)? {
                break;
            }
        }
    }

    let _ = stdout().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

pub(crate) fn command(
    logger: impl Logger + Sync + Clone,
    cache: &Cache,
    args: &RegistrySearchArgs,
) -> Result<ExitDirectives, DiagnosticMessages> {
    logger.loading(&format!("Resolving registry `{}`", args.registry.registry));

    let registry_id = "default";
    let registry_path =
        semconv_registry_path_from(&args.registry.registry, &args.registry.registry_git_sub_dir);

    // Load the semantic convention registry into a local cache.
    let semconv_specs = load_semconv_specs(&registry_path, cache, logger.clone())?;
    let mut registry = SemConvRegistry::from_semconv_specs(registry_id, semconv_specs);
    let schema = resolve_semconv_specs(&mut registry, logger.clone())?;

    // TODO - We should have two modes:
    // 1. An interactive UI
    // 2. a single input we take in and directly output some rendered result.
    run_ui(&schema).map_err(|e| DiagnosticMessages::from_error(e))?;
    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
