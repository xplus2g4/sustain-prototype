//! The read-only source viewer (FR-SRC-01..08, ADR 0009).

use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Arc, LazyLock};

use archalign_repo::RelPath;
use archalign_repo::text::LineIndex;
use iced::widget::text::{LineHeight, Span, Wrapping};
use iced::widget::{Space, button, column, container, rich_text, row, scrollable, span, text};
use iced::{Color, Element, Length, Padding};
use syntect::highlighting::{HighlightIterator, HighlightState, Highlighter, ThemeSet};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use crate::app::Message;
use crate::fonts::MONO;
use crate::ids;
use crate::palette::{Palette, ThemeChoice};
use crate::virtual_list::{self, ScrollState};

/// Height of one source line (fixed, for virtualization).
pub const LINE_H: f32 = 20.0;
/// Source font size.
pub const FONT_PX: f32 = 13.0;
/// JetBrains Mono advance at [`FONT_PX`].
pub const CHAR_W: f32 = FONT_PX * 0.6;
/// Lines between parse-state checkpoints.
const CHECKPOINT: usize = 128;
/// Highlighted lines kept in memory before the cache is reset.
const CACHE_LIMIT: usize = 20_000;

/// Syntect's default syntaxes (newline variant), loaded once.
pub static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
/// Syntect's default themes, loaded once.
pub static THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Picks a syntax by file extension (or full file name, e.g. `Makefile`), then by the
/// first line. `None` means plain text (FR-SRC-02).
pub fn select_syntax(path: &str, first_line: &str) -> Option<&'static SyntaxReference> {
    let ss: &'static SyntaxSet = &SYNTAXES;
    let p = Path::new(path);
    let by_ext = p
        .extension()
        .and_then(|e| e.to_str())
        .and_then(|e| ss.find_syntax_by_extension(e));
    let by_name = || {
        p.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| ss.find_syntax_by_extension(n))
    };
    by_ext
        .or_else(by_name)
        .or_else(|| ss.find_syntax_by_first_line(first_line))
        .filter(|s| s.name != "Plain Text")
}

fn theme_name(theme: ThemeChoice) -> &'static str {
    match theme {
        ThemeChoice::Dark => "base16-ocean.dark",
        ThemeChoice::Light => "InspiredGitHub",
    }
}

/// One colored piece of a line.
pub type Piece = (Color, String);

/// Incremental syntect highlighting with parse-state checkpoints.
#[derive(Debug, Clone)]
pub struct FileHighlighter {
    syntax: &'static SyntaxReference,
    theme: ThemeChoice,
    /// `checkpoints[k]` is the state before line `k * CHECKPOINT`.
    checkpoints: Vec<(ParseState, HighlightState)>,
    lines: HashMap<usize, Arc<Vec<Piece>>>,
    failed: bool,
}

impl FileHighlighter {
    /// A highlighter for a syntax and theme.
    pub fn new(syntax: &'static SyntaxReference, theme: ThemeChoice) -> Self {
        let mut h = Self {
            syntax,
            theme,
            checkpoints: Vec::new(),
            lines: HashMap::new(),
            failed: false,
        };
        h.reset_checkpoints();
        h
    }

    fn reset_checkpoints(&mut self) {
        let Some(theme) = THEMES.themes.get(theme_name(self.theme)) else {
            self.failed = true;
            return;
        };
        let highlighter = Highlighter::new(theme);
        self.checkpoints = vec![(
            ParseState::new(self.syntax),
            HighlightState::new(&highlighter, ScopeStack::new()),
        )];
        self.lines.clear();
    }

    /// The syntax name.
    pub fn syntax_name(&self) -> &str {
        &self.syntax.name
    }

    /// Switches the theme; cached colors are dropped.
    pub fn set_theme(&mut self, theme: ThemeChoice) {
        if self.theme != theme {
            self.theme = theme;
            self.reset_checkpoints();
        }
    }

    /// Cached pieces of a 0-based line.
    pub fn line(&self, n0: usize) -> Option<&Arc<Vec<Piece>>> {
        self.lines.get(&n0)
    }

    /// Highlights the given 0-based lines (and every line before them that hasn't been
    /// parsed yet, to build checkpoints).
    pub fn ensure(&mut self, text: &str, index: &LineIndex, range: Range<usize>) {
        if self.failed || range.is_empty() {
            return;
        }
        let range = range.start..range.end.min(index.line_count());
        if range.clone().all(|n| self.lines.contains_key(&n)) {
            return;
        }
        if self.lines.len() > CACHE_LIMIT {
            self.lines.clear();
        }
        let Some(theme) = THEMES.themes.get(theme_name(self.theme)) else {
            return;
        };
        let highlighter = Highlighter::new(theme);
        let k = (range.start / CHECKPOINT).min(self.checkpoints.len().saturating_sub(1));
        let Some((ps, hs)) = self.checkpoints.get(k).cloned() else {
            return;
        };
        let (mut ps, mut hs) = (ps, hs);
        let mut line_buf = String::new();
        for n in (k * CHECKPOINT)..range.end {
            if n % CHECKPOINT == 0 && n / CHECKPOINT == self.checkpoints.len() {
                self.checkpoints.push((ps.clone(), hs.clone()));
            }
            line_buf.clear();
            line_buf.push_str(index.line(text, n));
            line_buf.push('\n');
            let ops = match ps.parse_line(&line_buf, &SYNTAXES) {
                Ok(ops) => ops,
                Err(_) => {
                    self.failed = true;
                    return;
                }
            };
            if n >= range.start {
                let pieces: Vec<Piece> =
                    HighlightIterator::new(&mut hs, &ops, &line_buf, &highlighter)
                        .map(|(style, s)| {
                            let c = style.foreground;
                            (
                                Color::from_rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0),
                                s.trim_end_matches(['\n', '\r']).to_string(),
                            )
                        })
                        .filter(|(_, s)| !s.is_empty())
                        .collect();
                self.lines.insert(n, Arc::new(pieces));
            } else {
                // Advance the highlight state without keeping the output.
                for _ in HighlightIterator::new(&mut hs, &ops, &line_buf, &highlighter) {}
            }
        }
    }
}

/// A file shown as text.
#[derive(Debug, Clone)]
pub struct OpenText {
    /// Repo-relative path.
    pub rel: RelPath,
    /// Decoded text.
    pub text: Arc<String>,
    /// Line starts.
    pub index: Arc<LineIndex>,
    /// Whether invalid UTF-8 was replaced.
    pub lossy: bool,
    /// File size in bytes.
    pub size: u64,
    /// Longest line, in characters (for the row width).
    pub max_cols: usize,
    /// Syntax highlighting, when a syntax matched.
    pub highlighter: Option<FileHighlighter>,
}

/// Why a file is shown as a placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderReason {
    /// Larger than 2 MB.
    TooLarge,
    /// NUL byte in the first 8 KB.
    Binary,
}

/// What the source pane shows.
#[derive(Debug, Clone, Default)]
pub enum SourceContent {
    /// Nothing opened yet.
    #[default]
    Empty,
    /// A load is in flight.
    Loading(RelPath),
    /// A text file.
    Text(OpenText),
    /// A file that isn't displayed (FR-SRC-06).
    Placeholder {
        /// Path.
        rel: RelPath,
        /// Size in bytes.
        size: u64,
        /// Why.
        reason: PlaceholderReason,
    },
    /// The file could not be read.
    Error {
        /// Path.
        rel: RelPath,
        /// User-facing message.
        message: String,
    },
}

/// A request to show part of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cite {
    /// First cited line (1-based).
    pub line: Option<u64>,
    /// Last cited line (inclusive).
    pub end_line: Option<u64>,
}

/// State of the source pane.
#[derive(Debug, Clone, Default)]
pub struct SourceState {
    /// What is shown.
    pub content: SourceContent,
    /// Highlighted band, 1-based inclusive.
    pub band: Option<(usize, usize)>,
    /// Scroll position.
    pub scroll: ScrollState,
    /// Width of the viewport (rows are at least this wide so the band spans the pane).
    pub viewport_w: f32,
}

impl SourceState {
    /// Path of the shown file, if any.
    pub fn rel(&self) -> Option<&RelPath> {
        match &self.content {
            SourceContent::Empty => None,
            SourceContent::Loading(r) => Some(r),
            SourceContent::Text(t) => Some(&t.rel),
            SourceContent::Placeholder { rel, .. } | SourceContent::Error { rel, .. } => Some(rel),
        }
    }

    /// Number of lines of the shown text file.
    pub fn line_count(&self) -> Option<usize> {
        match &self.content {
            SourceContent::Text(t) => Some(t.index.line_count()),
            _ => None,
        }
    }

    /// The 0-based lines to build for the current scroll position (FR-SRC-03). `pane_h` is
    /// used until the scrollable reports its real height.
    pub fn window(&self, pane_h: f32) -> Range<usize> {
        virtual_list::window(
            self.scroll.or_height(pane_h),
            LINE_H,
            self.line_count().unwrap_or(0),
        )
    }

    /// Highlights the lines about to be built.
    pub fn ensure_highlighted(&mut self, pane_h: f32) {
        let window = self.window(pane_h);
        if let SourceContent::Text(t) = &mut self.content
            && let Some(h) = &mut t.highlighter
        {
            h.ensure(&t.text, &t.index, window);
        }
    }

    /// `path:line` for "Copy path:line" (just `path` without a band).
    pub fn copy_path_text(&self) -> Option<String> {
        let rel = self.rel()?;
        Some(match self.band {
            Some((l, _)) => format!("{rel}:{l}"),
            None => rel.to_string(),
        })
    }

    /// Text of the banded lines, for "Copy cited lines" (FR-SRC-07).
    pub fn cited_text(&self) -> Option<String> {
        let (a, b) = self.band?;
        let SourceContent::Text(t) = &self.content else {
            return None;
        };
        let n = t.index.line_count();
        if a == 0 || a > n {
            return None;
        }
        Some(t.index.range(&t.text, a - 1, b.min(n) - 1).to_string())
    }

    /// Switches the highlight theme.
    pub fn set_theme(&mut self, theme: ThemeChoice, pane_h: f32) {
        if let SourceContent::Text(t) = &mut self.content
            && let Some(h) = &mut t.highlighter
        {
            h.set_theme(theme);
        }
        self.ensure_highlighted(pane_h);
    }
}

/// Outcome of applying a citation to a file of `line_count` lines (FR-SRC-04, FR-SRC-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiteOutcome {
    /// No line given: open at line 1, no band.
    Top,
    /// Band over `start..=end` (1-based), scrolled to `start`.
    Band(usize, usize),
    /// The range is outside the file: open at line 1, no band, banner.
    OutOfRange {
        /// Cited first line.
        line: u64,
        /// Cited last line.
        end: u64,
        /// Lines in the file.
        lines: usize,
    },
}

/// Applies the citation rules.
pub fn apply_cite(cite: Cite, line_count: usize) -> CiteOutcome {
    let Some(line) = cite.line else {
        return CiteOutcome::Top;
    };
    let end = cite.end_line.unwrap_or(line);
    if line == 0 || end < line || end as usize > line_count {
        return CiteOutcome::OutOfRange {
            line,
            end,
            lines: line_count,
        };
    }
    CiteOutcome::Band(line as usize, end as usize)
}

/// Human-readable size.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / KB / KB)
    }
}

fn header<'a>(state: &'a SourceState, p: &'a Palette) -> Element<'a, Message> {
    let Some(rel) = state.rel() else {
        return Space::new().into();
    };
    let info = match &state.content {
        SourceContent::Text(t) => format!("{} lines", t.index.line_count()),
        SourceContent::Placeholder { size, .. } => format_size(*size),
        SourceContent::Loading(_) => "loading…".into(),
        _ => String::new(),
    };
    let small = |label: &'static str, msg: Option<Message>, id: iced::widget::Id| {
        container(
            button(text(label).size(11))
                .on_press_maybe(msg)
                .padding(Padding::from([2, 8]))
                .style(button::secondary),
        )
        .id(id)
    };
    let copy_lines = state.band.is_some().then_some(Message::CopyCitedLines);
    row![
        container(text(rel.to_string()).size(12).font(MONO).color(p.text)).id(ids::SRC_PATH),
        text(info).size(11).color(p.text_muted),
        Space::new().width(Length::Fill),
        small(
            "Copy path:line",
            Some(Message::CopyPathLine),
            ids::SRC_COPY_PATH
        ),
        small("Copy cited lines", copy_lines, ids::SRC_COPY_LINES),
    ]
    .spacing(8)
    .padding(Padding::from([4, 8]))
    .align_y(iced::Alignment::Center)
    .into()
}

/// The source pane body. `pane` is the pane's body size (fallback for the viewport).
pub fn view<'a>(state: &'a SourceState, p: &'a Palette, pane: iced::Size) -> Element<'a, Message> {
    let body: Element<'a, Message> = match &state.content {
        SourceContent::Empty => container(
            text("Click a file in the tree or a node in the diagram to open it here.")
                .size(12)
                .color(p.text_muted),
        )
        .padding(12)
        .into(),
        SourceContent::Loading(_) => container(text("Loading…").size(12).color(p.text_muted))
            .padding(12)
            .into(),
        SourceContent::Error { message, .. } => {
            container(text(message.clone()).size(12).color(p.danger()))
                .padding(12)
                .into()
        }
        SourceContent::Placeholder { size, reason, .. } => {
            let why = match reason {
                PlaceholderReason::TooLarge => "This file is larger than 2 MB and isn't displayed.",
                PlaceholderReason::Binary => "This looks like a binary file and isn't displayed.",
            };
            container(
                column![
                    text(why).size(12).color(p.text),
                    text(format!("Size: {}", format_size(*size)))
                        .size(12)
                        .color(p.text_muted),
                    container(
                        button(text("Copy path").size(11))
                            .on_press(Message::CopyPathLine)
                            .style(button::secondary)
                    )
                    .id(ids::SRC_COPY_PATH),
                ]
                .spacing(8),
            )
            .padding(12)
            .into()
        }
        SourceContent::Text(t) => lines_view(state, t, p, pane),
    };
    column![header(state, p), body].into()
}

fn lines_view<'a>(
    state: &'a SourceState,
    t: &'a OpenText,
    p: &'a Palette,
    pane: iced::Size,
) -> Element<'a, Message> {
    let total = t.index.line_count();
    let window = state.window(pane.height);
    let digits = total.max(1).to_string().len();
    let gutter_w = digits as f32 * CHAR_W + 16.0;
    let content_w = gutter_w + (t.max_cols as f32 + 4.0) * CHAR_W;
    let viewport_w = if state.viewport_w > 0.0 {
        state.viewport_w
    } else {
        pane.width
    };
    let row_w = content_w.max(viewport_w);
    let band_color = {
        let mut c = p.accent();
        c.a = 0.18;
        c
    };
    let mut col = column![Space::new().height(window.start as f32 * LINE_H)];
    for n0 in window.clone() {
        let n = n0 + 1;
        let in_band = state.band.is_some_and(|(a, b)| n >= a && n <= b);
        let spans: Vec<Span<'a, Message>> = match t.highlighter.as_ref().and_then(|h| h.line(n0)) {
            Some(pieces) => pieces
                .iter()
                .map(|(c, s)| span(s.clone()).color(*c).font(MONO))
                .collect(),
            None => vec![
                span(t.index.line(&t.text, n0).to_string())
                    .color(p.text)
                    .font(MONO),
            ],
        };
        let line = row![
            container(
                text(format!("{n:>digits$}"))
                    .size(FONT_PX)
                    .font(MONO)
                    .line_height(LineHeight::Absolute(LINE_H.into()))
                    .color(if in_band { p.text } else { p.text_dim })
            )
            .width(gutter_w)
            .padding(Padding::from([0, 8])),
            rich_text(spans)
                .size(FONT_PX)
                .font(MONO)
                .line_height(LineHeight::Absolute(LINE_H.into()))
                .wrapping(Wrapping::None),
        ];
        col = col.push(
            container(line)
                .id(ids::source_line(n))
                .width(row_w)
                .height(LINE_H)
                .style(move |_| container::Style {
                    background: in_band.then_some(iced::Background::Color(band_color)),
                    ..container::Style::default()
                }),
        );
    }
    col = col.push(Space::new().height((total - window.end) as f32 * LINE_H));
    scrollable(col)
        .id(ids::SOURCE_SCROLL)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .on_scroll(Message::SourceScrolled)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
