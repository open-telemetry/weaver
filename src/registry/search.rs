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
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Stylize, Terminal},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use std::io::stdout;
use tui_textarea::TextArea;

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
    search_area: TextArea<'a>,
}

impl<'a> SearchApp<'a> {
    fn new(schema: &'a ResolvedTelemetrySchema) -> SearchApp<'a> {
        let mut search_area = TextArea::default();
        search_area.set_placeholder_text("Enter search string");
        search_area.set_block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Gray))
                .title("Search (press `Esc` or `Ctrl-Q` to stop running) ")
                .title_style(Style::default().fg(Color::Green)),
        );
        SearchApp {
            schema,
            search_area,
        }
    }

    // Renders the title component of the UI.
    fn title(&self) -> Paragraph<'a> {
        let title_block = Block::default()
            .borders(Borders::TOP)
            .style(Style::default().bg(Color::Black))
            .border_style(Style::default().fg(Color::Gray))
            .title_alignment(ratatui::layout::Alignment::Center)
            .title_style(Style::default().fg(Color::Green))
            .title("Weaver Search");
        let title_contents = Line::from(vec![Span::styled(
            format!(
                "Loaded {0:?} registries w/ {1} attributes",
                self.schema.registries.keys(),
                self.schema.catalog.attributes.len()
            ),
            Style::default().fg(Color::Gray),
        )]);
        Paragraph::new(title_contents).block(title_block)
    }

    fn search_string(&self) -> String {
        self.search_area.lines().join(" ")
    }

    // Renders the current results of the search string or state of the UI.
    fn results(&self) -> Paragraph<'a> {
        let results: Vec<Line<'a>> = self
            .schema
            .catalog
            .attributes
            .iter()
            .filter(|a| a.name.contains(self.search_string().as_str()))
            .map(|a| {
                Line::default()
                    .style(Style::default().fg(Color::LightBlue))
                    .spans(vec![Span::raw(" - "), Span::raw(&a.name)])
            })
            .collect();
        let block = Block::new()
            .border_type(BorderType::Rounded)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .style(Style::default().bg(Color::Black))
            .title("Results [Attributes]");

        Paragraph::new(results).block(block)
    }

    // Creates the footer widget from current state.
    //
    // This should show the user what they're actively typing or offer help.
    fn footer(&self) -> &TextArea<'a> {
        &self.search_area
    }

    // Renders the text-UI to the current frame.
    fn render(&self, frame: &mut Frame<'_>) {
        // Set up the UI such that we have a title block,
        // a large section for results and then a footer with
        // information on how to get help or quit the application.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.size());
        frame.render_widget(self.title(), chunks[0]);
        frame.render_widget(self.results(), chunks[1]);
        frame.render_widget(self.footer().widget(), chunks[2]);
    }
    // Returns true when it's time to quit.
    fn process(&mut self, event: Event) -> Result<bool, Error> {
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('q')
                && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                return Ok(true);
            } else if key.code == KeyCode::Esc {
                return Ok(true);
            } else {
                let _ = self.search_area.input(event);
                // TODO - should we update search results here?
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
