// SPDX-License-Identifier: Apache-2.0

//! Search a semantic convention registry.

use clap::Args;
use itertools::Itertools;
use miette::Diagnostic;
use weaver_cache::Cache;
use weaver_common::diagnostic::DiagnosticMessages;
use weaver_common::Logger;
use weaver_resolved_schema::{attribute::Attribute, ResolvedTelemetrySchema};
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
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListState, Paragraph},
    Frame,
};
use std::io::{stdout, IsTerminal};
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

    /// An (optional) search string to use.  If specified, will return matching values on the command line.
    /// Otherwise, runs an interactive terminal UI.
    pub search_string: Option<String>,
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

// Our search application state.
// This will be updated by the `process` method, which handles events.
//
// It is then used to render in the `render` method, which we try to keep stateless.
struct SearchApp<'a> {
    // The current resolved schema that we should be searching.
    schema: &'a ResolvedTelemetrySchema,
    // A text-input area where users can enter a search string.
    search_area: TextArea<'a>,
    // The current selected index in search results.  Need to be manually cleared when new search strings are entered.
    selected_result_index: Option<usize>,
}

impl<'a> SearchApp<'a> {
    // Creates a new search application for a given resolved schema.
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
            selected_result_index: None,
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

    // Returns the current search string from the search widget state.
    fn search_string(&self) -> String {
        self.search_area.lines().join(" ")
    }

    // Returns a (not yet executed) iterator that will filter catalog attributes by the search string.
    fn result_set(&'a self) -> impl Iterator<Item = &'a Attribute> {
        self.schema
            .catalog
            .attributes
            .iter()
            .filter(|a| a.name.contains(self.search_string().as_str()))
    }

    // Returns a widget that will render the current results of all attributes which match the search string.
    fn results_widget(&'a self) -> List<'a> {
        let results: Vec<&'a str> = self.result_set().map(|a| a.name.as_str()).collect();
        let list = List::new(results)
            .block(
                Block::new()
                    .border_type(BorderType::Rounded)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
                    .style(Style::default().bg(Color::Black))
                    .title("Results [Attributes]"),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::ITALIC)
                    .bg(Color::DarkGray),
            )
            .highlight_symbol(">>")
            .repeat_highlight_symbol(true)
            .scroll_padding(2)
            .direction(ratatui::widgets::ListDirection::TopToBottom);
        list
    }

    // Returns a resulting attribute to display, if there are results *AND* a selected result on the result list.
    fn result(&'a self) -> Option<&'a Attribute> {
        self.selected_result_index
            .and_then(|idx| self.result_set().nth(idx))
    }

    // Returns the widget which displays details of the resulting attribute.
    // Note: this should be moved to a helper function and generaled to work on any selected result,
    // not just attributes.
    fn result_details_widget(&'a self) -> Paragraph<'a> {
        if let Some(result) = self.result() {
            let mut text = vec![
                Line::from(vec![
                    Span::styled("Id   : ", Style::default().fg(Color::Blue)),
                    Span::raw(result.name.to_owned()),
                ]),
                Line::from(vec![
                    Span::styled("Type : ", Style::default().fg(Color::Blue)),
                    Span::raw(result.r#type.to_string()),
                ]),
            ];
            // Tag
            if let Some(tag) = result.tag.as_ref() {
                text.push(Line::from(vec![
                    Span::styled("Tag  : ", Style::default().fg(Color::Blue)),
                    Span::raw(tag),
                ]));
            }

            // Brief
            // TODO - we can parse markdown and ANSI format.
            if !result.brief.trim().is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Brief: ",
                    Style::default().fg(Color::Blue),
                )));
                text.push(Line::from(result.brief.as_str()));
            }

            // Note
            if !result.note.trim().is_empty() {
                text.push(Line::from(""));
                text.push(Line::from(Span::styled(
                    "Note : ",
                    Style::default().fg(Color::Blue),
                )));
                text.push(Line::from(result.note.as_str()));
            }

            // Requirement Level
            text.push(Line::from(""));
            text.push(Line::from(vec![
                Span::styled("Requirement Level: ", Style::default().fg(Color::Blue)),
                Span::raw(format!("{}", result.requirement_level)),
            ]));

            // Stability level
            if let Some(stability) = result.stability.as_ref() {
                text.push(Line::from(vec![
                    Span::styled("Stability: ", Style::default().fg(Color::Blue)),
                    Span::raw(format!("{}", stability)),
                ]));
            }

            // Deprecation status.
            if let Some(deprecated) = result.deprecated.as_ref() {
                text.push(Line::from(vec![
                    Span::styled("Deprecated: ", Style::default().fg(Color::Blue)),
                    Span::raw(deprecated.to_string()),
                ]));
            }
            // Surround this paragraph of text with a border and description.
            Paragraph::new(text).block(
                Block::new()
                    .border_type(BorderType::Double)
                    .borders(Borders::all())
                    .border_style(Style::default().fg(Color::Green))
                    .title("Attribute"),
            )
        } else {
            Paragraph::new(Line::from("  Select a result to view details  "))
        }
    }

    // Creates the footer widget from current state.
    //
    // This should show the user what they're actively typing or offer help.
    fn footer(&self) -> &TextArea<'a> {
        &self.search_area
    }

    // Renders the text-UI to the current frame.
    //
    // This method should focus on LAYOUT of the user interface, and whether certian components are dispayed
    // at this time.
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

        // Render search reuslts.
        if let Some(index) = self.selected_result_index {
            // If the user is viewing a result, then we split the result window to show those results.
            let main_area = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(chunks[1]);
            //  Note - this is  hack around avoiding mutating list state in the render call...
            let mut result_state = ListState::default().with_selected(Some(index));
            frame.render_stateful_widget(self.results_widget(), main_area[0], &mut result_state);
            // Render the result details.
            frame.render_widget(self.result_details_widget(), main_area[1]);
        } else {
            frame.render_widget(self.results_widget(), chunks[1]);
        }

        // Render the footer.
        frame.render_widget(self.footer().widget(), chunks[2]);
    }

    // Processes events that will change the state of the UI.
    //
    // While we should likely encode a "focus" system where events get passed to certain widgets for handling, based on focus, for now
    // we try to keep all keys disjoint and handle them globally.
    //
    // This must return true (or an error) when it's time to quit.
    fn process(&mut self, event: Event) -> Result<bool, Error> {
        if let Event::Key(key) = event {
            match key.code {
                // Handle mechanisms to quite the UI.
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(true)
                }
                KeyCode::Esc => return Ok(true),
                // Handle all events that could scroll through search results.
                KeyCode::Up if key.kind == KeyEventKind::Press => self.move_index(-1),
                KeyCode::Down if key.kind == KeyEventKind::Press => self.move_index(1),
                // Send everything else to search input.  If search input handled the event, we clear the state of list selection.
                // This is likely too aggressive and we should check more nuanced changes before killing the state of the results.
                // We also could attempt to preserve the current index with the resulting list as much as feasible.
                _ => {
                    if self.search_area.input(event) {
                        self.selected_result_index = None;
                    }
                }
            }
        }
        Ok(false)
    }

    // Helper method for processing move events on the results list widget.
    fn move_index(&mut self, amt: i32) {
        let result_count = self.result_set().count();
        if let Some(value) = self.selected_result_index.as_mut() {
            *value = usize::min(i32::max(0, *value as i32 + amt) as usize, result_count - 1);
        } else {
            self.selected_result_index = Some(0);
        }
    }
}

// Boiler plate for running our ratatui UI.
//
// This sets up the terminal, and spins in the event loop processing keyboard (and other) events.
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

// If the user specified a search string on the command line, we operate as if we're a command-line tool, allowing
// awk/bash/etc type utilities on the result.
// TODO - the behavior of this method needs to be sorted out.
fn run_command_line_search(schema: &ResolvedTelemetrySchema, pattern: &str) {
    let results = schema
        .catalog()
        .attributes
        .iter()
        .filter(|a| a.name.contains(pattern))
        .map(|a| a.name.to_owned())
        .join("\n");
    println!("{}", results);
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

    // We should have two modes:
    // 1. a single input we take in and directly output some rendered result.
    // 2. An interactive UI
    if let Some(pattern) = args.search_string.as_ref() {
        run_command_line_search(&schema, pattern);
    } else if stdout().is_terminal() {
        run_ui(&schema).map_err(|e| DiagnosticMessages::from_error(e))?;
    } else {
        // TODO - custom error
        println!("Error: Could not find a terminal, and no search string was provided.");
        return Ok(ExitDirectives {
            exit_code: 1,
            quiet_mode: false,
        });
    }
    Ok(ExitDirectives {
        exit_code: 0,
        quiet_mode: false,
    })
}
