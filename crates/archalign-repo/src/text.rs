//! Reading source files for display (FR-SRC-06, FR-SRC-08, NFR-PERF-02).
//!
//! Reads go through [`Sandbox::resolve`]. Files over [`MAX_VIEW_BYTES`] are not read;
//! a NUL byte in the first [`BINARY_SNIFF_BYTES`] marks a file as binary; invalid UTF-8
//! is decoded lossily.

use std::fs::File;
use std::io::Read;

use crate::sandbox::{RelPath, Sandbox, SandboxError};

/// Largest file the viewer opens and a citation may point at: 2 MB.
pub const MAX_VIEW_BYTES: u64 = 2 * 1024 * 1024;

/// How many leading bytes are checked for a NUL byte.
pub const BINARY_SNIFF_BYTES: usize = 8 * 1024;

/// Byte offsets of the start of each line.
///
/// Line counting follows editors that don't count the phantom line after a final
/// newline: `""` has 0 lines, `"a"` 1, `"a\n"` 1, `"a\nb"` 2, `"a\n\n"` 2. Lines end at
/// `\n`; a `\r` before it is not part of the line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineIndex {
    starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    /// Indexes `text`.
    pub fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut starts = Vec::with_capacity(bytes.len() / 40 + 1);
        if !bytes.is_empty() {
            starts.push(0);
        }
        for (i, b) in bytes.iter().enumerate() {
            if *b == b'\n' && i + 1 < bytes.len() {
                starts.push(i + 1);
            }
        }
        Self {
            starts,
            len: bytes.len(),
        }
    }

    /// Number of lines (see the type docs for the counting rule).
    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    /// Byte range of line `n0` (0-based), without its line ending.
    fn span(&self, text: &str, n0: usize) -> Option<(usize, usize)> {
        let start = *self.starts.get(n0)?;
        let end = self.starts.get(n0 + 1).copied().unwrap_or(self.len);
        let bytes = text.as_bytes().get(start..end)?;
        let mut trimmed = bytes.len();
        if bytes.get(trimmed.wrapping_sub(1)) == Some(&b'\n') {
            trimmed -= 1;
            if bytes.get(trimmed.wrapping_sub(1)) == Some(&b'\r') {
                trimmed -= 1;
            }
        }
        Some((start, start + trimmed))
    }

    /// Line `n0` (0-based) without its trailing `\n` or `\r\n`; `""` when out of range
    /// or when `text` is not the indexed text.
    pub fn line<'t>(&self, text: &'t str, n0: usize) -> &'t str {
        self.span(text, n0)
            .and_then(|(a, b)| text.get(a..b))
            .unwrap_or("")
    }

    /// Lines `first0..=last0_inclusive` (0-based) joined by their original line endings,
    /// without the final line ending. `last0_inclusive` is clamped to the last line;
    /// `""` when the range is empty or out of range.
    pub fn range<'t>(&self, text: &'t str, first0: usize, last0_inclusive: usize) -> &'t str {
        let last = last0_inclusive.min(self.line_count().saturating_sub(1));
        if first0 > last {
            return "";
        }
        match (self.span(text, first0), self.span(text, last)) {
            (Some((a, _)), Some((_, b))) => text.get(a..b).unwrap_or(""),
            _ => "",
        }
    }
}

/// Counts lines in raw bytes with the same rule as [`LineIndex::line_count`].
pub fn count_lines(bytes: &[u8]) -> usize {
    let newlines = bytes.iter().filter(|b| **b == b'\n').count();
    match bytes.last() {
        None => 0,
        Some(b'\n') => newlines,
        Some(_) => newlines + 1,
    }
}

/// The result of opening a file for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextLoad {
    /// A text file.
    Text {
        /// The contents (lossily decoded when `lossy`).
        text: String,
        /// Whether invalid UTF-8 was replaced with U+FFFD.
        lossy: bool,
        /// Line starts of `text`.
        index: LineIndex,
        /// Size on disk in bytes.
        size: u64,
    },
    /// A NUL byte in the first 8 KB.
    Binary {
        /// Size in bytes.
        size: u64,
    },
    /// Larger than [`MAX_VIEW_BYTES`]; not read.
    TooLarge {
        /// Size in bytes.
        size: u64,
    },
}

/// Raw bytes of a file, or why they were not read.
pub(crate) enum RawLoad {
    /// The file's bytes (at most [`MAX_VIEW_BYTES`]) and its size.
    Bytes(Vec<u8>, u64),
    /// A NUL byte in the first 8 KB.
    Binary(u64),
    /// Over the size limit.
    TooLarge(u64),
}

/// Reads a regular file through the sandbox, enforcing the size limit and binary sniff.
pub(crate) fn read_raw(sandbox: &Sandbox, rel: &RelPath) -> Result<RawLoad, SandboxError> {
    let abs = sandbox.resolve(rel)?;
    let io_err = |e: std::io::Error| SandboxError::Io(format!("{rel}: {e}"));
    let meta = abs.metadata().map_err(io_err)?;
    if !meta.is_file() {
        return Err(SandboxError::InvalidPath(format!("{rel} is not a file")));
    }
    let size = meta.len();
    if size > MAX_VIEW_BYTES {
        return Ok(RawLoad::TooLarge(size));
    }
    let file = File::open(&abs).map_err(io_err)?;
    // The file may grow between `metadata` and the read: never read past the limit.
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
    file.take(MAX_VIEW_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io_err)?;
    let read = bytes.len() as u64;
    if read > MAX_VIEW_BYTES {
        return Ok(RawLoad::TooLarge(read));
    }
    let head = bytes.get(..BINARY_SNIFF_BYTES).unwrap_or(&bytes);
    if head.contains(&0) {
        return Ok(RawLoad::Binary(read));
    }
    Ok(RawLoad::Bytes(bytes, read))
}

/// Opens a file for the Source panel. Folders are rejected with
/// [`SandboxError::InvalidPath`].
pub fn load_text(sandbox: &Sandbox, rel: &RelPath) -> Result<TextLoad, SandboxError> {
    Ok(match read_raw(sandbox, rel)? {
        RawLoad::TooLarge(size) => TextLoad::TooLarge { size },
        RawLoad::Binary(size) => TextLoad::Binary { size },
        RawLoad::Bytes(bytes, size) => {
            let (text, lossy) = match String::from_utf8(bytes) {
                Ok(text) => (text, false),
                Err(err) => (String::from_utf8_lossy(err.as_bytes()).into_owned(), true),
            };
            let index = LineIndex::new(&text);
            TextLoad::Text {
                text,
                lossy,
                index,
                size,
            }
        }
    })
}
