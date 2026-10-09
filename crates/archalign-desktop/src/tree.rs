//! The project tree pane (FR-TREE-01..08).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;

use archalign_repo::RelPath;
use archalign_repo::tree::{DirEntry, EntryKind};
use iced::widget::{Space, button, column, container, row, scrollable, text, tooltip};
use iced::{Element, Length, Padding};

use crate::app::Message;
use crate::fonts::MONO;
use crate::ids;
use crate::palette::Palette;
use crate::virtual_list::{self, ScrollState};

/// Height of one tree row.
pub const ROW_H: f32 = 22.0;
/// Indentation per depth level.
pub const INDENT: f32 = 14.0;

/// One visible row of the flattened tree.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    /// Repo-relative path.
    pub rel: RelPath,
    /// Display name.
    pub name: String,
    /// Nesting depth (0 = top level).
    pub depth: usize,
    /// Folder, file or link.
    pub kind: EntryKind,
    /// Whether a folder row is expanded.
    pub expanded: bool,
}

impl TreeRow {
    /// Whether activating the row expands/collapses it.
    pub fn is_dir(&self) -> bool {
        matches!(
            self.kind,
            EntryKind::Dir
                | EntryKind::Symlink {
                    target_is_dir: true,
                    outside_root: false
                }
        )
    }

    /// A symlink that resolves outside the root: can't be expanded or opened.
    pub fn is_blocked(&self) -> bool {
        matches!(
            self.kind,
            EntryKind::Symlink {
                outside_root: true,
                ..
            }
        )
    }
}

/// Citation markers for the tree (FR-TREE-06), filled in by the diagram.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Markers {
    /// Files cited by the open diagram.
    pub cited: HashSet<String>,
    /// Folder → number of distinct cited files inside it.
    pub folder_counts: HashMap<String, usize>,
    /// Files with at least one broken citation.
    pub broken: HashSet<String>,
}

/// State of the tree pane.
#[derive(Debug, Clone, Default)]
pub struct TreeState {
    /// Listed folders and their entries ("" is the root).
    pub children: BTreeMap<RelPath, Vec<DirEntry>>,
    /// Expanded folders.
    pub expanded: HashSet<RelPath>,
    /// Folders whose listing is in flight.
    pub loading: HashSet<RelPath>,
    /// Listing errors per folder.
    pub errors: HashMap<RelPath, String>,
    /// Selected row.
    pub selected: Option<RelPath>,
    /// Scroll position.
    pub scroll: ScrollState,
    /// A path to reveal once its ancestors are listed (FR-TREE-05).
    pub pending_reveal: Option<RelPath>,
    /// Citation markers.
    pub markers: Markers,
}

impl TreeState {
    /// The flattened visible rows, depth-first.
    pub fn rows(&self) -> Vec<TreeRow> {
        let mut out = Vec::new();
        self.push_rows(&RelPath::root(), 0, &mut out);
        out
    }

    fn push_rows(&self, dir: &RelPath, depth: usize, out: &mut Vec<TreeRow>) {
        let Some(entries) = self.children.get(dir) else {
            return;
        };
        for e in entries {
            let expanded = self.expanded.contains(&e.rel);
            out.push(TreeRow {
                rel: e.rel.clone(),
                name: e.name.clone(),
                depth,
                kind: e.kind,
                expanded,
            });
            if expanded {
                self.push_rows(&e.rel, depth + 1, out);
            }
        }
    }

    /// Rows to build for the current scroll position (FR-TREE-03). `pane_h` is used until
    /// the scrollable reports its real height.
    pub fn window(&self, total: usize, pane_h: f32) -> Range<usize> {
        virtual_list::window(self.scroll.or_height(pane_h), ROW_H, total)
    }

    /// Ancestors of `rel` (outermost first), excluding the root and `rel` itself.
    pub fn ancestors(rel: &RelPath) -> Vec<RelPath> {
        let mut out = Vec::new();
        let mut cur = rel.parent();
        while let Some(p) = cur {
            if p.as_str().is_empty() {
                break;
            }
            cur = p.parent();
            out.push(p);
        }
        out.reverse();
        out
    }

    /// Continues a pending reveal: expands listed ancestors and returns the first ancestor
    /// that still needs listing, or `None` when the target row is reachable.
    pub fn advance_reveal(&mut self) -> Option<RelPath> {
        let target = self.pending_reveal.clone()?;
        for a in Self::ancestors(&target) {
            if !self.children.contains_key(&a) {
                if self.loading.contains(&a) {
                    return None;
                }
                return Some(a);
            }
            self.expanded.insert(a);
        }
        None
    }

    /// Whether the pending reveal is complete (all ancestors listed and expanded).
    pub fn reveal_ready(&self) -> bool {
        match &self.pending_reveal {
            None => false,
            Some(t) => Self::ancestors(t)
                .iter()
                .all(|a| self.children.contains_key(a) && self.expanded.contains(a)),
        }
    }

    /// Row index of a path among the visible rows.
    pub fn row_index(&self, rel: &RelPath) -> Option<usize> {
        self.rows().iter().position(|r| &r.rel == rel)
    }
}

fn marker_for<'a>(state: &TreeState, row: &TreeRow, p: &Palette) -> Option<Element<'a, Message>> {
    let m = &state.markers;
    let key = row.rel.as_str();
    if row.is_dir() {
        let n = m.folder_counts.get(key).copied().unwrap_or(0);
        return (n > 0).then(|| {
            text(format!("{n}"))
                .size(10)
                .font(MONO)
                .color(p.accent())
                .into()
        });
    }
    if m.broken.contains(key) {
        Some(text("⚠").size(11).color(p.warning()).into())
    } else if m.cited.contains(key) {
        Some(text("●").size(9).color(p.accent()).into())
    } else {
        None
    }
}

/// The tree pane body.
pub fn view<'a>(state: &'a TreeState, p: &'a Palette, pane_h: f32) -> Element<'a, Message> {
    let rows = state.rows();
    let total = rows.len();
    let window = state.window(total, pane_h);
    let top = window.start as f32 * ROW_H;
    let bottom = (total - window.end) as f32 * ROW_H;
    let mut col = column![Space::new().height(top)];
    for r in &rows[window.clone()] {
        col = col.push(row_view(state, r, p));
    }
    col = col.push(Space::new().height(bottom));
    let body: Element<'a, Message> = if total == 0 {
        let msg = if state.loading.contains(&RelPath::root()) {
            "Loading…"
        } else {
            "Empty folder"
        };
        container(text(msg).size(12).color(p.text_muted))
            .padding(10)
            .into()
    } else {
        scrollable(col.width(Length::Fill))
            .id(ids::TREE_SCROLL)
            .on_scroll(Message::TreeScrolled)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    };
    body
}

fn row_view<'a>(state: &'a TreeState, r: &TreeRow, p: &'a Palette) -> Element<'a, Message> {
    let selected = state.selected.as_ref() == Some(&r.rel);
    let icon = if r.is_dir() {
        if r.expanded { "▾ " } else { "▸ " }
    } else {
        "  "
    };
    let link = matches!(r.kind, EntryKind::Symlink { .. });
    let mut label = row![
        Space::new().width(INDENT * r.depth as f32),
        text(icon).size(12).font(MONO).color(p.text_muted),
        text(r.name.clone())
            .size(12)
            .font(MONO)
            .color(if r.is_blocked() { p.text_dim } else { p.text }),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);
    if link {
        label = label.push(text(" ↗").size(11).color(p.text_muted));
    }
    if let Some(m) = marker_for(state, r, p) {
        label = label.push(Space::new().width(6)).push(m);
    }
    let msg = if r.is_blocked() {
        None
    } else if r.is_dir() {
        Some(Message::TreeToggle(r.rel.clone()))
    } else {
        Some(Message::TreeActivate(r.rel.clone()))
    };
    let bg = if selected { Some(p.panel_border) } else { None };
    let btn = button(label)
        .on_press_maybe(msg)
        .padding(Padding::from([2, 6]))
        .width(Length::Fill)
        .height(ROW_H)
        .style(move |_t, status| {
            let hovered = matches!(status, button::Status::Hovered);
            button::Style {
                background: bg
                    .or(hovered.then_some(p.panel))
                    .map(iced::Background::Color),
                text_color: p.text,
                ..button::Style::default()
            }
        });
    let el: Element<'a, Message> = container(btn)
        .id(ids::tree_row(&r.rel))
        .width(Length::Fill)
        .into();
    if r.is_blocked() {
        tooltip(
            el,
            container(text("This link points outside the repository and can't be opened").size(11))
                .padding(6)
                .style(container::rounded_box),
            tooltip::Position::Bottom,
        )
        .into()
    } else {
        el
    }
}
