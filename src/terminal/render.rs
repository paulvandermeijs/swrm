use super::{Cell, CellFlags, Snapshot};
use gpui::{IntoElement, ParentElement, Styled, div, px, rgb};

/// Font size for the terminal grid. Used both by the renderer and by the
/// resize-to-bounds measurement in `TerminalTab::render`.
pub const CELL_FONT_SIZE_PX: f32 = 13.0;

/// Per-row line height in pixels. Pinned explicitly (rather than relying on
/// gpui's default `phi()` multiplier) so the resize-to-bounds math and the
/// renderer agree to within one pixel.
pub const CELL_LINE_HEIGHT_PX: f32 = 20.0;

pub fn render_snapshot(snap: &Snapshot) -> impl IntoElement {
    let cols = snap.cols as usize;
    let mut col = div()
        .flex()
        .flex_col()
        .font_family("JetBrains Mono")
        .text_size(px(CELL_FONT_SIZE_PX))
        .line_height(px(CELL_LINE_HEIGHT_PX));

    for row in 0..snap.rows as usize {
        let start = row * cols;
        let row_cells = &snap.cells[start..start + cols];
        col = col.child(render_row(row_cells));
    }
    col
}

fn render_row(cells: &[Cell]) -> impl IntoElement {
    let mut row = div().flex().flex_row();
    if cells.is_empty() {
        return row;
    }

    let mut run_start = 0;
    for i in 1..=cells.len() {
        let same_run = i < cells.len()
            && cells[i].fg == cells[run_start].fg
            && cells[i].bg == cells[run_start].bg
            && cells[i].flags == cells[run_start].flags;
        if !same_run {
            let run = &cells[run_start..i];
            row = row.child(render_run(run));
            run_start = i;
        }
    }
    row
}

fn render_run(run: &[Cell]) -> impl IntoElement {
    debug_assert!(!run.is_empty());
    let head = &run[0];
    let text: String = run.iter().map(|c| c.ch).collect();

    let mut el = div().text_color(rgb(head.fg)).bg(rgb(head.bg)).child(text);

    if head.flags.contains(CellFlags::BOLD) {
        el = el.font_weight(gpui::FontWeight::BOLD);
    }
    if head.flags.contains(CellFlags::ITALIC) {
        el = el.italic();
    }
    if head.flags.contains(CellFlags::UNDERLINE) {
        el = el.underline();
    }
    el
}
