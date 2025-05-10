use clap_markdown::{help_markdown_custom, MarkdownOptions};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn run() -> anyhow::Result<()> {
    println!("Generating Markdown documentation...");

    // Get markdown content for CLI
    let markdown =
        help_markdown_custom::<weaver_cli::cli::Cli>(&MarkdownOptions::new().show_footer(false));

    let output_dir = Path::new("docs");
    // Define output file path
    let output_file = output_dir.join("usage.md");

    // Write markdown to file
    let mut file = File::create(&output_file)?;
    file.write_all(markdown.as_bytes())?;

    Ok(())
}
