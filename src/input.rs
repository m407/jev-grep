//! Bounded readers for physical text and JSONL records.
use std::io::{BufRead, Read};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::fragment::Record;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFormat {
    Text,
    LogJsonl,
    AstGrep,
}

#[derive(Clone, Debug)]
pub struct CodeCandidate {
    pub text: String,
    pub file: String,
    pub language: Option<String>,
    pub start_line: u64,
    pub end_line: u64,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// Decode and validate one native ast-grep scan --json=stream object.
pub fn parse_ast_grep(record: &Record, name: &str) -> Result<CodeCandidate> {
    validate_ast_grep(record)
        .with_context(|| format!("{name}:{}: invalid ast-grep JSONL candidate", record.line))
}

fn validate_ast_grep(record: &Record) -> Result<CodeCandidate> {
    let value: serde_json::Value = serde_json::from_str(&record.text).context("invalid JSON")?;
    let obj = value
        .as_object()
        .context("ast-grep record must be an object")?;
    let text = obj
        .get("text")
        .and_then(|v| v.as_str())
        .context("ast-grep text must be a string")?
        .to_owned();
    ensure!(!text.contains('\0'), "candidate text contains NUL");
    let file = obj
        .get("file")
        .and_then(|v| v.as_str())
        .context("ast-grep file must be a string")?
        .to_owned();
    ensure!(!file.is_empty(), "ast-grep file must not be empty");
    let range = obj
        .get("range")
        .and_then(|v| v.as_object())
        .context("ast-grep range must be an object")?;
    let bytes = range
        .get("byteOffset")
        .and_then(|v| v.as_object())
        .context("ast-grep range.byteOffset must be an object")?;
    let start_byte = coordinate(bytes.get("start"), "range.byteOffset.start", record.line)?;
    let end_byte = coordinate(bytes.get("end"), "range.byteOffset.end", record.line)?;
    ensure!(
        start_byte <= end_byte && end_byte - start_byte == text.len() as u64,
        "ast-grep byte range does not match UTF-8 text length"
    );
    let start = range
        .get("start")
        .and_then(|v| v.as_object())
        .context("ast-grep range.start must be an object")?;
    let end = range
        .get("end")
        .and_then(|v| v.as_object())
        .context("ast-grep range.end must be an object")?;
    let start_line0 = coordinate(start.get("line"), "range.start.line", record.line)?;
    let end_line0 = coordinate(end.get("line"), "range.end.line", record.line)?;
    let start_column = coordinate(start.get("column"), "range.start.column", record.line)?;
    let end_column = coordinate(end.get("column"), "range.end.column", record.line)?;
    ensure!(start_line0 <= end_line0, "ast-grep line range is reversed");
    let start_line = start_line0
        .checked_add(1)
        .context("ast-grep start line overflow")?;
    let line_breaks = text.bytes().filter(|b| *b == b'\n').count() as u64;
    let expected_end = start_line0
        .checked_add(line_breaks)
        .context("ast-grep end line overflow")?;
    ensure!(
        end_line0 == expected_end,
        "ast-grep line range does not match text"
    );
    let final_line_chars = text.rsplit('\n').next().unwrap_or_default().chars().count() as u64;
    let expected_end_column = if start_line0 == end_line0 {
        start_column
            .checked_add(final_line_chars)
            .context("ast-grep end column overflow")?
    } else {
        final_line_chars
    };
    ensure!(
        end_column == expected_end_column,
        "ast-grep columns do not match UTF-8 text"
    );
    let end_line = if text.ends_with('\n') {
        end_line0
    } else {
        end_line0
            .checked_add(1)
            .context("ast-grep end line overflow")?
    };
    let language = obj
        .get("language")
        .map(|v| v.as_str().context("ast-grep language must be a string"))
        .transpose()?
        .map(str::to_owned);
    Ok(CodeCandidate {
        text,
        file,
        language,
        start_line,
        end_line,
        start_byte,
        end_byte,
    })
}

fn coordinate(value: Option<&serde_json::Value>, field: &str, line: u64) -> Result<u64> {
    value
        .and_then(serde_json::Value::as_u64)
        .with_context(|| format!("{field} must be a nonnegative integer at JSONL line {line}"))
}

/// Reads one physical record at a time, never retaining more than the
/// configured line limit plus one byte used to detect overflow.
pub struct Reader<'a> {
    input: &'a mut dyn BufRead,
    name: &'a str,
    format: InputFormat,
    max_line_bytes: u64,
    line: u64,
    offset: u64,
}

impl<'a> Reader<'a> {
    pub fn new(
        input: &'a mut dyn BufRead,
        name: &'a str,
        format: InputFormat,
        max_line_bytes: u64,
    ) -> Result<Self> {
        ensure!(max_line_bytes > 0, "--max-line-bytes must be positive");
        Ok(Self {
            input,
            name,
            format,
            max_line_bytes,
            line: 0,
            offset: 0,
        })
    }

    pub fn next_record(&mut self) -> Result<Option<Record>> {
        let mut raw = Vec::new();
        let limit = self
            .max_line_bytes
            .checked_add(1)
            .context("--max-line-bytes overflow")?;
        let read = (&mut self.input)
            .take(limit)
            .read_until(b'\n', &mut raw)
            .with_context(|| format!("{}:{}: reading input", self.name, self.line + 1))?;
        if read == 0 {
            return Ok(None);
        }
        let line = self
            .line
            .checked_add(1)
            .context("input line number overflow")?;
        ensure!(
            u64::try_from(raw.len()).unwrap_or(u64::MAX) <= self.max_line_bytes,
            "{}:{line}: line exceeds --max-line-bytes (including newline)",
            self.name
        );
        let start = self.offset;
        let end = start
            .checked_add(u64::try_from(raw.len()).context("input record too large")?)
            .context("input byte offset overflow")?;

        let text = match self.format {
            InputFormat::Text | InputFormat::AstGrep => {
                ensure!(
                    !raw.contains(&0),
                    "{}:{line}: binary NUL byte encountered",
                    self.name
                );
                String::from_utf8(raw.clone())
                    .with_context(|| format!("{}:{line}: input is not UTF-8", self.name))?
            }
            InputFormat::LogJsonl => {
                let physical = strip_line_ending(&raw);
                ensure!(
                    !physical.contains(&0),
                    "{}:{line}: binary NUL byte encountered",
                    self.name
                );
                let physical = std::str::from_utf8(physical)
                    .with_context(|| format!("{}:{line}: JSONL input is not UTF-8", self.name))?;
                ensure!(
                    physical.trim_start().starts_with('{'),
                    "{}:{line}: log-jsonl record must be a JSON object",
                    self.name
                );
                let value: LogRecord = serde_json::from_str(physical)
                    .with_context(|| format!("{}:{line}: invalid log-jsonl record", self.name))?;
                ensure!(
                    !value.text.contains('\0'),
                    "{}:{line}: text contains NUL",
                    self.name
                );
                value.text
            }
        };
        self.line = line;
        self.offset = end;
        Ok(Some(Record {
            text,
            raw,
            line,
            byte_range: start..end,
        }))
    }
}

#[derive(Deserialize)]
struct LogRecord {
    text: String,
}

fn strip_line_ending(raw: &[u8]) -> &[u8] {
    let without_lf = raw.strip_suffix(b"\n").unwrap_or(raw);
    without_lf.strip_suffix(b"\r").unwrap_or(without_lf)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::fragment::{Fragment, TextBasis};

    fn records(input: &[u8], format: InputFormat, max: u64) -> Result<Vec<Record>> {
        let mut source = Cursor::new(input);
        let mut reader = Reader::new(&mut source, "fixture", format, max)?;
        let mut records = Vec::new();
        while let Some(record) = reader.next_record()? {
            records.push(record);
        }
        Ok(records)
    }

    #[test]
    fn text_keeps_exact_bytes_crlf_and_eof_without_lf() {
        let input = "λ\r\nlast".as_bytes();
        let records = records(input, InputFormat::Text, 16).unwrap();
        assert_eq!(records[0].raw, "λ\r\n".as_bytes());
        assert_eq!(
            &input[records[0].byte_range.start as usize..records[0].byte_range.end as usize],
            records[0].raw
        );
        assert_eq!(records[1].raw, b"last");
        assert_eq!(
            &input[records[1].byte_range.start as usize..records[1].byte_range.end as usize],
            records[1].raw
        );
        let mut fragment = Fragment::from_record(&records[0], TextBasis::Source);
        fragment.append(&records[1]);
        assert_eq!(fragment.text.as_bytes(), input);
        assert_eq!(fragment.byte_range, 0..input.len() as u64);
    }

    #[test]
    fn rejects_nul_invalid_utf8_and_oversized_physical_lines() {
        assert!(records(b"a\0\n", InputFormat::Text, 10).is_err());
        assert!(records(b"\xff\n", InputFormat::Text, 10).is_err());
        assert!(records(b"1234\n", InputFormat::Text, 4).is_err());
        assert!(Reader::new(&mut Cursor::new(b""), "fixture", InputFormat::Text, 0).is_err());
    }

    #[test]
    fn jsonl_escaped_newlines_are_decoded_without_changing_physical_coordinates() {
        let input = b"{\"text\":\"first\\nsecond\",\"other\":1}\r\n{\"text\":\"last\"}";
        let records = records(input, InputFormat::LogJsonl, 100).unwrap();
        assert_eq!(records[0].text, "first\nsecond");
        assert_eq!(records[0].line, 1);
        assert_eq!(records[0].byte_range, 0..36);
        assert_eq!(records[1].line, 2);
        assert_eq!(records[1].byte_range, 36..input.len() as u64);
        let mut fragment = Fragment::from_record(&records[0], TextBasis::DecodedRecords);
        fragment.append(&records[1]);
        assert_eq!(fragment.text, "first\nsecond\nlast");
        assert_eq!(fragment.text_basis.as_str(), "decoded-records");
        assert_eq!((fragment.start_line, fragment.end_line), (1, 2));
        assert_eq!(
            &input[fragment.byte_range.start as usize..fragment.byte_range.end as usize],
            input
        );
    }

    #[test]
    fn malformed_jsonl_shapes_and_decoded_nul_are_rejected() {
        for input in [
            &b"[]\n"[..],
            &b"[\"x\"]\n"[..],
            &b"{\"text\":4}\n"[..],
            &b"{\"other\":\"x\"}\n"[..],
            &b"{\"text\":\"\\u0000\"}\n"[..],
            &b"{\"text\":\"x\"}\xff"[..],
        ] {
            assert!(
                records(input, InputFormat::LogJsonl, 100).is_err(),
                "{input:?}"
            );
        }
    }

    #[test]
    fn physical_line_and_byte_coordinate_overflow_are_checked() {
        let mut source = Cursor::new(b"x\n");
        let mut reader = Reader::new(&mut source, "fixture", InputFormat::Text, 8).unwrap();
        reader.offset = u64::MAX;
        assert!(reader.next_record().is_err());

        let mut source = Cursor::new(b"x\n");
        let mut reader = Reader::new(&mut source, "fixture", InputFormat::Text, 8).unwrap();
        reader.line = u64::MAX;
        assert!(reader.next_record().is_err());
    }
}
