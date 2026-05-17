use super::{Cell, CellFlags, Snapshot};
use gpui::{
    App, Bounds, FontStyle, FontWeight, IntoElement, Pixels, Point, Styled, TextRun, Window,
    canvas, fill, font, point, px, rgb, size,
};

/// Font size for the terminal grid.
pub const CELL_FONT_SIZE_PX: f32 = 13.0;

/// Per-row line height in pixels. Sized to fully contain the natural extent of
/// Powerline / Nerd-Font fallback glyphs (e.g. `` arrow) at this font
/// size — at 17px the arrow's top edge poked 1px above the cell.
pub const CELL_LINE_HEIGHT_PX: f32 = 18.0;

pub fn render_snapshot(snap: &Snapshot, cell_width: f32) -> impl IntoElement {
    // Clone so the snapshot can move into the canvas paint closure (which is `FnOnce + 'static`).
    let snap = snap.clone();
    canvas(
        |_bounds, _window, _cx| {},
        move |bounds, _state, window, cx| {
            paint_grid(&snap, bounds.origin, cell_width, window, cx);
        },
    )
    .size_full()
}

fn paint_grid(
    snap: &Snapshot,
    origin: Point<Pixels>,
    cell_width: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let cols = snap.cols as usize;
    if cols == 0 {
        return;
    }
    let rows = snap.rows as usize;
    let cw = px(cell_width);

    for row in 0..rows {
        let row_cells = &snap.cells[row * cols..(row + 1) * cols];
        let y = origin.y + px(row as f32 * CELL_LINE_HEIGHT_PX);
        paint_row_bg(row_cells, origin.x, y, cell_width, window);
        paint_row_text(row_cells, origin.x, y, cw, window, cx);
    }
}

fn paint_row_bg(cells: &[Cell], x0: Pixels, y: Pixels, cell_width: f32, window: &mut Window) {
    let lh = px(CELL_LINE_HEIGHT_PX);
    let mut start = 0;
    for i in 1..=cells.len() {
        let same = i < cells.len() && cells[i].bg == cells[start].bg;
        if !same {
            let x = x0 + px(start as f32 * cell_width);
            let w = px((i - start) as f32 * cell_width);
            window.paint_quad(fill(
                Bounds::new(point(x, y), size(w, lh)),
                rgb(cells[start].bg),
            ));
            start = i;
        }
    }
}

fn paint_row_text(
    cells: &[Cell],
    x0: Pixels,
    y: Pixels,
    cell_width: Pixels,
    window: &mut Window,
    cx: &mut App,
) {
    let mut text = String::with_capacity(cells.len());
    let mut runs: Vec<TextRun> = Vec::new();
    let mut start = 0;

    for i in 1..=cells.len() {
        let same = i < cells.len()
            && cells[i].fg == cells[start].fg
            && cells[i].flags == cells[start].flags;
        if !same {
            let mut byte_len = 0;
            for cell in &cells[start..i] {
                text.push(cell.ch);
                byte_len += cell.ch.len_utf8();
            }
            let head = &cells[start];
            runs.push(TextRun {
                len: byte_len,
                font: cell_font(head.flags),
                color: rgb(head.fg).into(),
                background_color: None,
                underline: head.flags.contains(CellFlags::UNDERLINE).then(|| {
                    gpui::UnderlineStyle {
                        color: None,
                        thickness: px(1.0),
                        wavy: false,
                    }
                }),
                strikethrough: None,
            });
            start = i;
        }
    }

    let shaped = window.text_system().shape_line(
        text.into(),
        px(CELL_FONT_SIZE_PX),
        &runs,
        Some(cell_width),
    );
    let _ = shaped.paint(
        point(x0, y),
        px(CELL_LINE_HEIGHT_PX),
        gpui::TextAlign::Left,
        None,
        window,
        cx,
    );
}

fn cell_font(flags: CellFlags) -> gpui::Font {
    let mut f = font("JetBrains Mono");
    if flags.contains(CellFlags::BOLD) {
        f.weight = FontWeight::BOLD;
    }
    if flags.contains(CellFlags::ITALIC) {
        f.style = FontStyle::Italic;
    }
    f
}
