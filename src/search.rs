use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::io::{BufRead, Write};

use crate::{
    Args,
    api::Api,
    fragment::{Fragment, TextBasis},
    input::{InputFormat, Reader},
};

#[derive(Serialize)]
struct Source<'a> {
    kind: &'static str,
    path: Option<&'a str>,
}
#[derive(Serialize)]
struct Range {
    start_line: u64,
    end_line: u64,
    start_byte: u64,
    end_byte: u64,
}
#[derive(Serialize)]
struct Located<'a> {
    kind: &'static str,
    source: Source<'a>,
    range: Range,
    text: &'a str,
    text_basis: &'a str,
    probability: f64,
    split_before: bool,
    split_after: bool,
}

#[derive(Default)]
pub struct CodeSourceAccounting {
    sources: Vec<(String, u64, bool)>,
    indexes: std::collections::HashMap<String, usize>,
}

impl CodeSourceAccounting {
    pub fn write_counts(&self, output: &mut dyn Write) -> Result<()> {
        for (source, count, _) in &self.sources {
            writeln!(output, "{source}:{count}")?;
        }
        output.flush()?;
        Ok(())
    }
}

pub fn search(
    reader: &mut dyn BufRead,
    name: &str,
    _prefix: bool,
    args: &Args,
    api: &Api,
    output: &mut dyn Write,
    code_sources: &mut CodeSourceAccounting,
) -> Result<bool> {
    ensure!(
        args.input_format == "text"
            || args.input_format == "log-jsonl"
            || args.input_format == "ast-grep",
        "unsupported input format"
    );
    let format = if args.input_format == "ast-grep" {
        InputFormat::AstGrep
    } else if args.input_format == "log-jsonl" {
        InputFormat::LogJsonl
    } else {
        InputFormat::Text
    };
    let basis = if format == InputFormat::Text {
        TextBasis::Source
    } else {
        TextBasis::DecodedRecords
    };
    let mut reader = Reader::new(reader, name, format, args.max_line_bytes)?;
    let mut pending: Option<(Fragment, f64, bool)> = None;
    let mut selected = 0u64;
    let mut found = false;
    // ast-grep's `file` accounting is shared across all physical inputs in
    // this invocation; each input remains a separate JSONL transport.
    while let Some(record) = reader.next_record()? {
        if format == InputFormat::AstGrep {
            let candidate = crate::input::parse_ast_grep(&record, name)?;
            ensure!(
                candidate.text.len() as u64 <= args.max_fragment_bytes,
                "{}:{}: ast-grep candidate exceeds --max-fragment-bytes; reduce the upstream candidate or raise the limit",
                name,
                record.line
            );
            let source_index = if let Some(&index) = code_sources.indexes.get(&candidate.file) {
                index
            } else {
                let index = code_sources.sources.len();
                code_sources
                    .sources
                    .push((candidate.file.clone(), 0, false));
                code_sources.indexes.insert(candidate.file.clone(), index);
                index
            };
            let source_selected = code_sources.sources[source_index].1;
            // A capped source is still fully parsed and size-checked above,
            // but must not trigger another inference request.
            if !args.max_count.is_some_and(|limit| source_selected >= limit) {
                let probability = api
                    .probability(args, &candidate.text)
                    .with_context(|| format!("{name}:{}", record.line))?;
                let was_listed = code_sources.sources[source_index].2;
                if emit_code(
                    &candidate,
                    probability,
                    name,
                    args,
                    output,
                    &mut code_sources.sources[source_index].1,
                    was_listed,
                )? {
                    found = true;
                    if args.files_with_matches {
                        code_sources.sources[source_index].2 = true;
                    }
                }
            }
            if found && args.quiet {
                return Ok(found);
            }
            continue;
        }
        ensure!(
            record.text.len() as u64 <= args.max_fragment_bytes,
            "{name}:{}: record exceeds --max-fragment-bytes",
            record.line
        );
        if let Some((mut f, p, split_before)) = pending.take() {
            let separator = usize::from(basis == TextBasis::DecodedRecords);
            if f.text
                .len()
                .saturating_add(separator)
                .saturating_add(record.text.len()) as u64
                > args.max_fragment_bytes
            {
                if emit(&f, p, split_before, true, name, args, output, &mut selected)? {
                    found = true;
                }
                if args.files_with_matches && found {
                    return Ok(true);
                }
                if (args.quiet && found) || (args.max_count.is_some_and(|n| selected >= n)) {
                    if args.count {
                        writeln!(output, "{name}:{selected}")?;
                        output.flush()?;
                    }
                    return Ok(found);
                }
                let next = api
                    .probability(args, &record.text)
                    .with_context(|| format!("{name}:{}", record.line))?;
                pending = Some((Fragment::from_record(&record, basis), next, true));
            } else {
                let scores = api
                    .group_probabilities(args, &f.text, &record.text)
                    .with_context(|| format!("{name}:{}", record.line))?;
                if scores.same_event >= args.join_threshold
                    || scores.useful_context >= args.join_threshold
                {
                    f.append(&record);
                    pending = Some((f, scores.match_extended, split_before));
                } else {
                    if emit(
                        &f,
                        p,
                        split_before,
                        false,
                        name,
                        args,
                        output,
                        &mut selected,
                    )? {
                        found = true;
                    }
                    if args.files_with_matches && found {
                        return Ok(true);
                    }
                    if (args.quiet && found) || (args.max_count.is_some_and(|n| selected >= n)) {
                        if args.count {
                            writeln!(output, "{name}:{selected}")?;
                            output.flush()?;
                        }
                        return Ok(found);
                    }
                    pending = Some((
                        Fragment::from_record(&record, basis),
                        scores.match_next,
                        false,
                    ));
                }
            }
        } else {
            let p = api
                .probability(args, &record.text)
                .with_context(|| format!("{name}:{}", record.line))?;
            pending = Some((Fragment::from_record(&record, basis), p, false));
        }
    }
    if format == InputFormat::AstGrep {
        return Ok(found);
    }
    if let Some((f, p, before)) = pending
        && emit(&f, p, before, false, name, args, output, &mut selected)?
    {
        found = true;
    }
    if args.count {
        writeln!(output, "{name}:{selected}")?;
        output.flush()?;
    }
    Ok(found)
}

fn emit_code(
    candidate: &crate::input::CodeCandidate,
    probability: f64,
    _input_name: &str,
    args: &Args,
    out: &mut dyn Write,
    count: &mut u64,
    was_listed: bool,
) -> Result<bool> {
    if (probability >= args.threshold) != !args.invert_match {
        return Ok(false);
    }
    *count += 1;
    if args.quiet {
        return Ok(true);
    }
    if args.files_with_matches {
        if !was_listed {
            writeln!(out, "{}", candidate.file)?;
            out.flush()?;
        }
        return Ok(true);
    }
    if args.count {
        return Ok(true);
    }
    if args.json || args.format.as_deref() == Some("json") {
        #[derive(Serialize)]
        struct CodeLocated<'a> {
            kind: &'static str,
            source: Source<'a>,
            range: Range,
            text: &'a str,
            text_basis: &'static str,
            probability: f64,
            split_before: bool,
            split_after: bool,
            #[serde(skip_serializing_if = "Option::is_none")]
            language: Option<&'a str>,
        }
        serde_json::to_writer(
            &mut *out,
            &CodeLocated {
                kind: "code",
                source: if candidate.file == "STDIN" {
                    Source {
                        kind: "stdin",
                        path: None,
                    }
                } else {
                    Source {
                        kind: "file",
                        path: Some(&candidate.file),
                    }
                },
                range: Range {
                    start_line: candidate.start_line,
                    end_line: candidate.end_line,
                    start_byte: candidate.start_byte,
                    end_byte: candidate.end_byte,
                },
                text: &candidate.text,
                text_basis: "source",
                probability,
                split_before: false,
                split_after: false,
                language: candidate.language.as_deref(),
            },
        )?;
        writeln!(out)?;
    } else {
        writeln!(
            out,
            "{}:{}-{} bytes {}-{}",
            if candidate.file == "STDIN" {
                "(stdin)"
            } else {
                &candidate.file
            },
            candidate.start_line,
            candidate.end_line,
            candidate.start_byte,
            candidate.end_byte
        )?;
        write!(out, "{}", candidate.text)?;
        if !candidate.text.ends_with('\n') {
            writeln!(out)?;
        }
    }
    out.flush()?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn emit(
    f: &Fragment,
    p: f64,
    before: bool,
    after: bool,
    name: &str,
    args: &Args,
    out: &mut dyn Write,
    count: &mut u64,
) -> Result<bool> {
    if (p >= args.threshold) == !args.invert_match { /* selected */
    } else {
        return Ok(false);
    }
    *count += 1;
    if args.quiet {
        return Ok(true);
    }
    if args.files_with_matches {
        writeln!(out, "{name}")?;
        out.flush()?;
        return Ok(true);
    }
    if args.count {
        return Ok(true);
    }
    if args.json || args.format.as_deref() == Some("json") {
        let source = if name == "(standard input)" {
            Source {
                kind: "stdin",
                path: None,
            }
        } else {
            Source {
                kind: "file",
                path: Some(name),
            }
        };
        serde_json::to_writer(
            &mut *out,
            &Located {
                kind: "log",
                source,
                range: Range {
                    start_line: f.start_line,
                    end_line: f.end_line,
                    start_byte: f.byte_range.start,
                    end_byte: f.byte_range.end,
                },
                text: &f.text,
                text_basis: f.text_basis.as_str(),
                probability: p,
                split_before: before,
                split_after: after,
            },
        )?;
        writeln!(out)?;
    } else {
        writeln!(
            out,
            "{name}:{}-{}{}{}{}",
            f.start_line,
            f.end_line,
            if f.text_basis == TextBasis::DecodedRecords {
                " (JSONL line coordinates)"
            } else {
                ""
            },
            if before { " [split-before]" } else { "" },
            if after { " [split-after]" } else { "" }
        )?;
        write!(out, "{}", f.text)?;
        if !f.text.ends_with('\n') {
            writeln!(out)?;
        }
    }
    out.flush()?;
    Ok(true)
}
