// SPDX-License-Identifier: Apache-2.0

//! Command to generate a client SDK.

use std::path::PathBuf;

use clap::Parser;

use weaver_logger::Logger;
use weaver_template::sdkgen::ClientSdkGenerator;
use weaver_template::GeneratorConfig;

/// Parameters for the `gen-client-sdk` command
#[derive(Parser)]
pub struct GenClientCommand {
    /// Schema file to resolve
    #[arg(short, long, value_name = "FILE")]
    schema: PathBuf,

    /// Language to generate the client SDK for
    #[arg(short, long)]
    language: String,

    /// Output directory where the client API will be generated
    #[arg(short, long, value_name = "DIR")]
    output_dir: PathBuf,
}

/// Generate a client SDK (application)
pub fn command_gen_client(log: impl Logger + Sync + Clone, params: &GenClientCommand) {
    log.loading(&format!(
        "Generating client SDK for language {}",
        params.language
    ));
    let generator = match ClientSdkGenerator::try_new(&params.language, GeneratorConfig::default())
    {
        Ok(gen) => gen,
        Err(e) => {
            log.error(&format!("{}", e));
            std::process::exit(1);
        }
    };

    generator
        .generate(
            log.clone(),
            params.schema.clone(),
            params.output_dir.clone(),
        )
        .map_err(|e| {
            log.error(&format!("{}", e));
            std::process::exit(1);
        })
        .unwrap();

    log.success("Generated client SDK");
}
