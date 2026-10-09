//! Editing helpers and serialization.

use crate::types::ArchitectureDoc;

/// Strips authored geometry before Re-layout (ADR 0019): component `pos`, `size`, `row`,
/// `col`, the `layout` block, and the connection routing hints.
pub fn strip_geometry(doc: &mut ArchitectureDoc) {
    doc.layout = None;
    for c in &mut doc.components {
        c.pos = None;
        c.size = None;
        c.row = None;
        c.col = None;
    }
    if let Some(conns) = &mut doc.connections {
        for c in conns {
            c.via = None;
            c.from_side = None;
            c.to_side = None;
            c.route = None;
            c.label_at = None;
            c.label_dx = None;
            c.label_dy = None;
            c.label_segment = None;
        }
    }
}

/// Serializes a document pretty-printed with 2-space indentation and a trailing newline.
/// Whole numbers are written as integers (FR-VAL-03).
pub fn to_pretty_json(doc: &ArchitectureDoc) -> Vec<u8> {
    // The model holds only strings, integers, finite floats, booleans and maps with string
    // keys, all of which serde_json can always serialize.
    let mut out = serde_json::to_vec_pretty(doc).unwrap_or_default();
    out.push(b'\n');
    out
}
