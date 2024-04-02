// SPDX-License-Identifier: Apache-2.0

//! Command to generate a client SDK.

use std::io;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use crossterm::{
    event::{self, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::prelude::{CrosstermBackend, Span, Terminal};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Cell;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, STORED, TEXT};
use tantivy::{Index, IndexWriter, ReloadPolicy};
use tui_textarea::TextArea;

use theme::ThemeConfig;
use weaver_cache::Cache;
use weaver_logger::Logger;
use weaver_resolver::attribute::AttributeCatalog;
use weaver_resolver::registry::resolve_semconv_registry;
use weaver_resolver::SchemaResolver;
use weaver_schema::attribute::Attribute;
use weaver_schema::TelemetrySchema;

use crate::search::schema::{attribute, metric, metric_group, resource, span};

mod schema;
mod semconv;
mod theme;

type Err = Box<dyn std::error::Error>;
type Result<T> = std::result::Result<T, Err>;

/// Parameters for the `search` command
#[derive(Debug, Args)]
pub struct SearchCommand {
    /// Define the sub-commands for the `search` command
    #[clap(subcommand)]
    pub command: SearchSubCommand,
}

/// Sub-commands for the `search` command
#[derive(Debug, Subcommand)]
pub enum SearchSubCommand {
    /// Search in a semantic convention registry
    Registry(SearchRegistry),
    /// Search in a semantic convention registry [WIP, todo]
    Registry2(SearchRegistry2),
    /// Search in a telemetry schema
    Schema(SearchSchema),
}

/// Parameters for the `search registry` sub-command
#[derive(Debug, Args)]
pub struct SearchRegistry {
    /// Git URL of the semantic convention registry
    pub registry: String,

    /// Optional path in the git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,

    /// The telemetry schema containing the versions (url or file)
    #[arg(short, long)]
    schema: Option<String>,
}

/// Parameters for the `search registry` sub-command [WIP, todo]
#[derive(Debug, Args)]
pub struct SearchRegistry2 {
    /// Git URL of the semantic convention registry
    pub registry: String,

    /// Optional path in the git repository where the semantic convention
    /// registry is located
    pub path: Option<String>,

    /// The telemetry schema containing the versions (url or file)
    #[arg(short, long)]
    schema: Option<String>,
}

/// Parameters for the `search schema` sub-command
#[derive(Debug, Args)]
pub struct SearchSchema {
    /// Schema file to search
    pub schema: PathBuf,
}

pub struct SearchApp<'a> {
    schema: TelemetrySchema,
    search_area: TextArea<'a>,

    results: StatefulResults,

    searcher: tantivy::Searcher,
    query_parser: QueryParser,
    current_query: Option<String>,

    should_quit: bool,

    theme: ThemeConfig,
}

/// A result item
pub struct ResultItem {
    path: String,
    brief: String,
}

/// A stateful list of items
pub struct StatefulResults {
    state: TableState,
    // ListState,
    items: Vec<ResultItem>,
}

/// A struct representing all the fields in an indexed document.
pub struct DocFields {
    path: Field,
    brief: Field,
    note: Field,
    tag: Field,
}

impl StatefulResults {
    /// Creates a new stateful list of items
    fn new() -> StatefulResults {
        StatefulResults {
            state: TableState::default(), // ListState::default(),
            items: vec![],
        }
    }

    /// Selects the next item in the list
    fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Selects the previous item in the list
    fn previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    /// Unselects the current selection
    fn unselect(&mut self) {
        self.state.select(None);
    }

    /// Clears the results
    fn clear(&mut self) {
        self.unselect();
        self.items.clear();
    }
}

/// Search for attributes and metrics in a schema file
#[cfg(not(tarpaulin_include))]
pub fn command_search(log: impl Logger + Sync + Clone, command: &SearchCommand) {
    let cache = Cache::try_new().unwrap_or_else(|e| {
        log.error(&e.to_string());
        std::process::exit(1);
    });

    match &command.command {
        SearchSubCommand::Registry(args) => search_registry_command(log, &cache, args),
        SearchSubCommand::Registry2(args) => search_registry_command2(log, &cache, args),
        SearchSubCommand::Schema(args) => search_schema_command(log, &cache, args),
    }
}

/// Search semantic convention registry command [todo, WIP].
#[cfg(not(tarpaulin_include))]
fn search_registry_command2(
    log: impl Logger + Sync + Clone,
    cache: &Cache,
    registry_args: &SearchRegistry2,
) {
    let registry_id = "default";
    let semconv_specs = SchemaResolver::load_semconv_registry(
        registry_id,
        registry_args.registry.clone(),
        registry_args.path.clone(),
        cache,
        log.clone(),
    )
    .unwrap_or_else(|e| {
        log.error(&format!("{}", e));
        std::process::exit(1);
    });

    let mut attr_catalog = AttributeCatalog::default();
    let resolved_registry =
        resolve_semconv_registry(&mut attr_catalog, &registry_args.registry, &semconv_specs)
            .unwrap_or_else(|e| {
                log.error(&format!("{}", e));
                std::process::exit(1);
            });

    dbg!(resolved_registry);
    //dbg!(attr_catalog);

    // let schema = if let Some(schema) = &registry_args.schema {
    //     let mut schema =
    //         SchemaResolver::resolve_schema(schema, cache, log.clone()).unwrap_or_else(|e| {
    //             log.error(&format!("{}", e));
    //             std::process::exit(1);
    //         });
    //     schema.semantic_convention_registry = semconv_registry;
    //     schema
    // } else {
    //     TelemetrySchema {
    //         file_format: "".to_string(),
    //         parent_schema_url: None,
    //         schema_url: "".to_string(),
    //         semantic_conventions: vec![],
    //         schema: None,
    //         versions: None,
    //         parent_schema: None,
    //         semantic_convention_registry: semconv_registry,
    //     }
    // };
    //
    // search_schema_tui(log, schema);
}

/// Search semantic convention registry command.
#[cfg(not(tarpaulin_include))]
fn search_registry_command(
    log: impl Logger + Sync + Clone,
    cache: &Cache,
    registry_args: &SearchRegistry,
) {
    let registry_id = "default";
    let semconv_registry = SchemaResolver::resolve_semconv_registry(
        registry_id,
        registry_args.registry.clone(),
        registry_args.path.clone(),
        cache,
        log.clone(),
    )
    .unwrap_or_else(|e| {
        log.error(&format!("{}", e));
        std::process::exit(1);
    });

    let schema = if let Some(schema) = &registry_args.schema {
        let mut schema =
            SchemaResolver::resolve_schema(schema, cache, log.clone()).unwrap_or_else(|e| {
                log.error(&format!("{}", e));
                std::process::exit(1);
            });
        schema.semantic_convention_registry = semconv_registry;
        schema
    } else {
        TelemetrySchema {
            file_format: "".to_owned(),
            parent_schema_url: None,
            schema_url: "".to_owned(),
            semantic_conventions: vec![],
            schema: None,
            versions: None,
            parent_schema: None,
            semantic_convention_registry: semconv_registry,
        }
    };

    search_schema_tui(log, schema);
}

/// Search schema command.
#[cfg(not(tarpaulin_include))]
fn search_schema_command(
    log: impl Logger + Sync + Clone,
    cache: &Cache,
    schema_args: &SearchSchema,
) {
    let schema =
        SchemaResolver::resolve_schema_file(schema_args.schema.clone(), cache, log.clone())
            .unwrap_or_else(|e| {
                log.error(&format!("{}", e));
                std::process::exit(1);
            });

    search_schema_tui(log, schema);
}

#[cfg(not(tarpaulin_include))]
fn search_schema_tui(log: impl Logger + Sync + Clone, schema: TelemetrySchema) {
    let semconv_registry = schema.semantic_convention_catalog();

    let mut schema_builder = Schema::builder();
    let fields = DocFields {
        path: schema_builder.add_text_field("path", TEXT | STORED),
        brief: schema_builder.add_text_field("brief", TEXT | STORED),
        note: schema_builder.add_text_field("note", TEXT),
        tag: schema_builder.add_text_field("tag", TEXT),
    };

    let index_schema = schema_builder.build();
    let index = Index::create_in_ram(index_schema.clone());
    let mut index_writer: IndexWriter = index
        .writer(15_000_000)
        .expect("Failed to create index writer");

    attribute::index_semconv_attributes(
        semconv_registry.attributes_iter(),
        "semconv",
        &fields,
        &mut index_writer,
    );
    metric::index_semconv_metrics(
        semconv_registry.metrics_iter(),
        "semconv",
        &fields,
        &mut index_writer,
    );
    resource::index(&schema, &fields, &mut index_writer);
    metric::index_schema_metrics(&schema, &fields, &mut index_writer);
    metric_group::index(&schema, &fields, &mut index_writer);
    schema::event::index(&schema, &fields, &mut index_writer);
    span::index(&schema, &fields, &mut index_writer);

    _ = index_writer
        .commit()
        .expect("Failed to commit index writer");
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into()
        .expect("Failed to create reader");
    let searcher = reader.searcher();
    let DocFields {
        path,
        brief,
        note,
        tag,
    } = fields;
    let query_parser = QueryParser::for_index(&index, vec![path, brief, note, tag]);

    let theme = ThemeConfig {
        title: Color::Rgb(238, 238, 238),
        border: Color::Rgb(85, 109, 89),
        label: Color::Rgb(128, 208, 163),
        value: Color::Rgb(204, 204, 204),
    };

    let mut search_area = TextArea::default();
    search_area.set_cursor_line_style(Style::default());
    search_area.set_placeholder_text("Enter search terms, operators, or use path:, brief:, tag:, or note: prefixes to target specific fields.");
    search_area.set_block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme.border))
            .title("Search (press `Esc` or `Ctrl-C` to stop running) ")
            .title_style(Style::default().fg(theme.title)),
    );

    // application state
    let mut app = SearchApp {
        schema,
        search_area,
        results: StatefulResults::new(),
        searcher,
        query_parser,
        current_query: None,
        should_quit: false,
        theme,
    };

    search_tui(&mut app).unwrap_or_else(|e| {
        log.error(&format!("{}", e));
        std::process::exit(1);
    });
}

#[cfg(not(tarpaulin_include))]
fn search_tui(app: &mut SearchApp<'_>) -> Result<()> {
    // Startup
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let status = run(app);

    // Shutdown
    disable_raw_mode()?;
    execute!(
        term.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    term.show_cursor()?;

    status?;
    Ok(())
}

fn ui(app: &mut SearchApp<'_>, frame: &mut Frame<'_>) {
    let empty_search_box = app.search_area.is_empty();
    app.search_area.lines().iter().for_each(|query| {
        if let Some(current_query) = app.current_query.as_ref() {
            if current_query == query {
                return;
            }
        }
        app.current_query = Some(query.to_string());
        match app.query_parser.parse_query(query) {
            Ok(query) => {
                app.results.clear();
                let top_docs = app
                    .searcher
                    .search(&query, &TopDocs::with_limit(100))
                    .expect("Failed to search");
                for (_score, doc_address) in top_docs {
                    let retrieved_doc = app
                        .searcher
                        .doc(doc_address)
                        .expect("Failed to retrieve document");
                    let values = retrieved_doc.field_values();
                    let path = values[0].value().as_text().unwrap_or_default();
                    let brief = values[1].value().as_text().unwrap_or_default();

                    app.results.items.push(ResultItem {
                        path: path.to_owned(),
                        brief: brief.to_owned(),
                    });
                }
                app.results.next();
            }
            Err(_e) => {
                app.results.clear();
            }
        }
    });

    let selected_style = Style::default()
        .bg(Color::Rgb(106, 47, 47))
        .fg(app.theme.title);
    let normal_style = Style::default();
    let header_cells = ["Path:", "Brief:"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(app.theme.title)));
    let header = Row::new(header_cells)
        .style(normal_style)
        .height(1)
        .bottom_margin(0);
    let rows: Vec<Row<'_>> = app
        .results
        .items
        .iter()
        .map(|item| {
            let cells = vec![
                Cell::from(item.path.clone()).fg(app.theme.label),
                Cell::from(item.brief.clone()).fg(app.theme.value),
            ];
            Row::new(cells).height(1).bottom_margin(0)
        })
        .collect();

    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(frame.size());

    let inner_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(outer_layout[0]);

    let content = Table::new(rows, [Constraint::Max(50), Constraint::Max(120)])
        .header(header)
        .block(
            Block::default()
                //.borders(Borders::TOP.union(Borders::RIGHT))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.border))
                .title("Search results ")
                .title_style(Style::default().fg(app.theme.value)),
        )
        .highlight_style(selected_style)
        .highlight_symbol(">> ");

    frame.render_stateful_widget(content, inner_layout[1], &mut app.results.state);

    // Detail area
    let item = match app.results.state.selected() {
        Some(i) => app.results.items.get(i),
        None => None,
    };
    if empty_search_box {
        frame.render_widget(summary_area(app), inner_layout[0]);
    } else {
        frame.render_widget(detail_area(app, item), inner_layout[0]);
    }
    frame.render_widget(app.search_area.widget(), outer_layout[1]);
}

#[cfg(not(tarpaulin_include))]
fn summary_area<'a>(app: &'a SearchApp<'a>) -> Paragraph<'a> {
    let area_title = "Summary";
    let semconv_catalog = app.schema.semantic_convention_catalog();
    let text = vec![
        Line::from(""),
        Line::from("Telemetry schema:"),
        Line::from(format!("- URL: {}", app.schema.schema_url)),
        Line::from(format!("- Parent schema URL: {}", app.schema.parent_schema_url.clone().unwrap_or_default())),
        Line::from(format!("- {} metrics", app.schema.metrics_count())),
        Line::from(format!("- {} metric groups", app.schema.metric_groups_count())),
        Line::from(format!("- {} events", app.schema.events_count())),
        Line::from(format!("- {} spans", app.schema.spans_count())),
        Line::from(format!("- {} versions", app.schema.version_count())),
        Line::from(""),
        Line::from(vec![
            Span::raw("Semantic convention catalog:"),
        ]),
        Line::from(vec![
            Span::raw(format!("- {} files.", semconv_catalog.asset_count())),
        ]),
        Line::from(vec![
            Span::raw(format!("- {} attributes.", semconv_catalog.attribute_count())),
        ]),
        Line::from(vec![
            Span::raw(format!("- {} metrics.", semconv_catalog.metric_count())),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(">> Enter search terms, operators, or use path:, brief:, tag:, or note: prefixes to target specific fields."),
    ];

    let paragraph = Paragraph::new(text).style(Style::default().fg(app.theme.value));

    paragraph
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.border))
                .title(format!("{} ", area_title))
                .title_style(Style::default().fg(app.theme.title))
                .style(Style::default()),
        )
        .wrap(Wrap { trim: true })
}

#[cfg(not(tarpaulin_include))]
fn detail_area<'a>(app: &'a SearchApp<'a>, item: Option<&'a ResultItem>) -> Paragraph<'a> {
    let mut area_title = "Details";
    let paragraph = if let Some(item) = item {
        let path = item.path.as_str().split('/').collect::<Vec<&str>>();

        match path[..] {
            ["semconv", "attr", id] => {
                area_title = "Semantic Convention Attribute";
                semconv::attribute::widget(
                    app.schema
                        .semantic_convention_catalog()
                        .attribute_with_provenance(id),
                    &app.theme,
                )
            }
            ["semconv", "metric", id] => {
                area_title = "Semantic Convention Metric";
                semconv::metric::widget(
                    app.schema
                        .semantic_convention_catalog()
                        .metric_with_provenance(id),
                    &app.theme,
                )
            }
            ["schema", "resource", "attr", attr_id] => {
                area_title = "Schema Resource Attribute";
                if let Some(resource) = app.schema.resource() {
                    attribute::widget(
                        resource.attributes.iter().find(|attr| {
                            if let Attribute::Id { id, .. } = attr {
                                id.as_str() == attr_id
                            } else {
                                false
                            }
                        }),
                        app.schema.schema_url.as_str(),
                        &app.theme,
                    )
                } else {
                    Paragraph::new(vec![Line::default()])
                }
            }
            ["schema", "metric", id] => {
                area_title = "Schema Metric";
                metric::widget(
                    app.schema.metric(id),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "metric", metric_id, "attr", attr_id] => {
                area_title = "Schema Metric Attribute";
                attribute::widget(
                    app.schema
                        .metric(metric_id)
                        .iter()
                        .flat_map(|m| m.attribute(attr_id))
                        .next(),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "metric_group", id] => {
                area_title = "Schema Metric Group";
                metric_group::widget(
                    app.schema.metric_group(id),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "metric_group", metric_group_id, "attr", attr_id] => {
                area_title = "Schema Metric Group";
                attribute::widget(
                    app.schema
                        .metric_group(metric_group_id)
                        .iter()
                        .flat_map(|m| m.attribute(attr_id))
                        .next(),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "event", id] => {
                area_title = "Schema Event";
                schema::event::widget(
                    app.schema.event(id),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "event", event_id, "attr", attr_id] => {
                area_title = "Schema Event Attribute";
                attribute::widget(
                    app.schema
                        .event(event_id)
                        .iter()
                        .flat_map(|m| m.attribute(attr_id))
                        .next(),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "span", id] => {
                area_title = "Schema Span";
                span::widget(
                    app.schema.span(id),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            ["schema", "span", span_id, "attr", attr_id] => {
                area_title = "Schema Span Attribute";
                attribute::widget(
                    app.schema
                        .span(span_id)
                        .iter()
                        .flat_map(|m| m.attribute(attr_id))
                        .next(),
                    app.schema.schema_url.as_str(),
                    &app.theme,
                )
            }
            _ => Paragraph::new(vec![Line::default()]),
        }
    } else {
        Paragraph::new(vec![Line::default()])
    };

    paragraph
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.theme.border))
                .title(format!("{} ", area_title))
                .title_style(Style::default().fg(app.theme.title))
                //.padding(Padding::new(1,0,0,0))
                .style(Style::default()),
        )
        .wrap(Wrap { trim: true })
}

#[cfg(not(tarpaulin_include))]
fn update(app: &mut SearchApp<'_>) -> Result<()> {
    if event::poll(std::time::Duration::from_millis(250))? {
        let event = event::read()?;
        if let event::Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Esc => {
                        app.should_quit = true;
                        return Ok(());
                    }
                    KeyCode::Up => app.results.previous(),
                    KeyCode::Down => app.results.next(),
                    KeyCode::Enter => {}
                    _ => {
                        _ = app.search_area.input(event);
                    }
                }
            }
        }
        // app.search_area.input(event);
    }

    Ok(())
}

#[cfg(not(tarpaulin_include))]
fn run(app: &mut SearchApp<'_>) -> Result<()> {
    // ratatui terminal
    let mut t = Terminal::new(CrosstermBackend::new(io::stderr()))?;

    loop {
        // application render
        _ = t.draw(|f| {
            ui(app, f);
        })?;

        // application update
        update(app)?;

        // application exit
        if app.should_quit {
            break;
        }
    }

    Ok(())
}
