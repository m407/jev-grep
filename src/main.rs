mod api;
mod fragment;
mod input;
mod rate_limit;
mod search;

use anyhow::{Context, Result, ensure};
use clap::Parser;
use ignore::{WalkBuilder, overrides::OverrideBuilder};
use std::fs::File;
use std::io::{self, BufReader, IsTerminal};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "Streaming semantic grep via Jev on OpenCode Zen")]
pub struct Args {
    /// Natural-language criterion, sent as the typed question instructions
    query: String,
    /// Files/directories; '-' reads stdin. Default: stdin if piped, otherwise '.'
    paths: Vec<PathBuf>,
    /// Full System One endpoint (input text is sent here)
    #[arg(
        long,
        env = "JEV_GREP_API_URL",
        default_value = "https://opencode.ai/zen/v1/systemone"
    )]
    api_url: String,
    /// Bearer token; prefer the environment variable to avoid shell history exposure
    #[arg(long, env = "JEV_GREP_API_KEY", hide_env_values = true)]
    api_key: Option<String>,
    /// Jev model ID
    #[arg(long, env = "JEV_GREP_MODEL", default_value = "jev-1.13")]
    model: String,
    /// Select fragments whose P(true) is at least this value
    #[arg(long, default_value_t = 0.5)]
    threshold: f64,
    /// Join adjacent records when either relation score reaches this value
    #[arg(long, default_value_t = 0.5)]
    join_threshold: f64,
    /// Input format
    #[arg(long, value_parser = ["text", "log-jsonl", "ast-grep"], default_value = "text")]
    input_format: String,
    /// Output format
    #[arg(long, value_parser = ["human", "json"])]
    format: Option<String>,
    #[arg(short = 'n', long)]
    line_number: bool,
    #[arg(short = 'v', long)]
    invert_match: bool,
    #[arg(short = 'c', long, conflicts_with_all = ["json", "format", "files_with_matches", "quiet"])]
    count: bool,
    #[arg(short = 'l', long, conflicts_with_all = ["json", "format", "quiet"])]
    files_with_matches: bool,
    #[arg(short = 'q', long, conflicts_with_all = ["json", "format"])]
    quiet: bool,
    /// Emit selected lines as JSONL (not ripgrep's event schema)
    #[arg(long)]
    json: bool,
    /// Stop after this many selected fragments per input
    #[arg(short = 'm', long)]
    max_count: Option<u64>,
    #[arg(short = 'g', long = "glob")]
    globs: Vec<String>,
    #[arg(long)]
    hidden: bool,
    #[arg(long)]
    no_ignore: bool,
    #[arg(short = 'H', long, conflicts_with = "no_filename")]
    with_filename: bool,
    #[arg(long)]
    no_filename: bool,
    /// Maximum bytes per input line, including line ending
    #[arg(long, default_value_t = 16384)]
    max_line_bytes: u64,
    /// Maximum evaluated fragment bytes
    #[arg(long, default_value_t = 16384)]
    max_fragment_bytes: u64,
    /// Local tokens/minute budget (60-second sliding window); unset disables it
    #[arg(long, env = "JEV_GREP_TPM")]
    tpm: Option<u64>,
    /// Conservative request-token estimate multiplier (finite and at least 1)
    #[arg(long, default_value_t = 1.2)]
    tpm_safety_factor: f64,
    /// Evaluation timeout including retries/reading, excluding TPM and HTTP 429 waits
    #[arg(long, default_value_t = 30)]
    timeout: u64,
}

fn run(mut args: Args) -> Result<bool> {
    ensure!(!args.query.trim().is_empty(), "query must not be empty");
    ensure!(!args.model.trim().is_empty(), "model must not be empty");
    ensure!(
        args.threshold.is_finite() && (0.0..=1.0).contains(&args.threshold),
        "threshold must be between 0 and 1"
    );
    ensure!(
        args.join_threshold.is_finite() && (0.0..=1.0).contains(&args.join_threshold),
        "join-threshold must be between 0 and 1"
    );
    ensure!(
        args.max_fragment_bytes > 0,
        "max-fragment-bytes must be positive"
    );
    ensure!(
        !args.no_filename,
        "--no-filename is incompatible: located output always includes source"
    );
    ensure!(
        !(args.json && args.format.is_some()),
        "--json conflicts with --format"
    );
    ensure!(
        args.max_line_bytes > 0 && args.max_line_bytes < u64::MAX,
        "max-line-bytes must be positive and below u64::MAX"
    );
    ensure!(args.timeout > 0, "timeout must be positive");
    ensure!(args.tpm != Some(0), "tpm must be positive");
    ensure!(
        args.tpm_safety_factor.is_finite() && args.tpm_safety_factor >= 1.0,
        "tpm-safety-factor must be finite and at least 1"
    );
    ensure!(args.max_count != Some(0), "max-count must be positive");
    if args.paths.is_empty() {
        args.paths
            .push(if io::stdin().is_terminal() { "." } else { "-" }.into());
    }
    ensure!(
        args.paths.iter().filter(|p| p.as_os_str() == "-").count() <= 1,
        "stdin may only be specified once"
    );
    let prefix = !args.no_filename
        && (args.with_filename || args.paths.len() > 1 || args.paths.iter().any(|p| p.is_dir()));
    let api = api::Api::new(&args)?;
    let mut output = io::stdout().lock();
    let mut found = false;
    let mut code_sources = search::CodeSourceAccounting::default();
    for path in &args.paths {
        if path.as_os_str() == "-" {
            found |= search::search(
                &mut io::stdin().lock(),
                "(standard input)",
                prefix,
                &args,
                &api,
                &mut output,
                &mut code_sources,
            )?;
        } else {
            let mut overrides = OverrideBuilder::new(std::env::current_dir()?);
            for glob in &args.globs {
                overrides.add(glob).context("invalid glob")?;
            }
            let mut builder = WalkBuilder::new(path);
            let overrides = overrides.build()?;
            builder.hidden(!args.hidden).follow_links(false);
            if args.no_ignore {
                builder
                    .ignore(false)
                    .git_ignore(false)
                    .git_global(false)
                    .git_exclude(false)
                    .parents(false);
            }
            for entry in builder.build() {
                let entry = entry.with_context(|| format!("walking {}", path.display()))?;
                if !entry.file_type().is_some_and(|t| t.is_file()) {
                    continue;
                }
                if overrides.matched(entry.path(), false).is_ignore() {
                    continue;
                }
                let name = entry.path().to_string_lossy();
                let file = File::open(entry.path()).with_context(|| format!("opening {name}"))?;
                found |= search::search(
                    &mut BufReader::new(file),
                    &name,
                    prefix,
                    &args,
                    &api,
                    &mut output,
                    &mut code_sources,
                )?;
                if args.quiet && found {
                    return Ok(true);
                }
            }
        }
        if args.quiet && found {
            return Ok(true);
        }
    }
    if args.input_format == "ast-grep" && args.count {
        code_sources.write_counts(&mut output)?;
    }
    Ok(found)
}

fn main() {
    let code = match run(Args::parse()) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(error) => {
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
            }) {
                0
            } else {
                eprintln!("jev-grep: {error:#}");
                2
            }
        }
    };
    std::process::exit(code);
}
