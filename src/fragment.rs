//! Byte-oriented input locations shared by readers and later fragment assembly.
//!
//! Raw `source` text is an exact source byte slice (including CRLF/LF, when
//! present). `decoded-records` text is instead the decoded JSONL `text` values,
//! joined with one LF; its range always describes the physical JSONL records.
use std::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextBasis {
    Source,
    DecodedRecords,
}

impl TextBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::DecodedRecords => "decoded-records",
        }
    }
}

/// One validated physical input record. Coordinates are 1-based inclusive
/// lines and 0-based half-open byte offsets into the input stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub text: String,
    pub raw: Vec<u8>,
    pub line: u64,
    pub byte_range: Range<u64>,
}

/// Text plus the physical source span that produced it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fragment {
    pub text: String,
    pub text_basis: TextBasis,
    pub start_line: u64,
    pub end_line: u64,
    pub byte_range: Range<u64>,
}

impl Fragment {
    pub fn from_record(record: &Record, text_basis: TextBasis) -> Self {
        Self {
            text: record.text.clone(),
            text_basis,
            start_line: record.line,
            end_line: record.line,
            byte_range: record.byte_range.clone(),
        }
    }

    /// Append a logical record. JSONL decoded records receive exactly one LF;
    /// raw source records already carry their original physical separators.
    pub fn append(&mut self, record: &Record) {
        if self.text_basis == TextBasis::DecodedRecords {
            self.text.push('\n');
        }
        self.text.push_str(&record.text);
        self.end_line = record.line;
        self.byte_range.end = record.byte_range.end;
    }
}
