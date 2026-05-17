use super::Snapshot;
use gpui::{IntoElement, ParentElement, Styled, div, px, rgb};

pub fn render_snapshot(snap: &Snapshot) -> impl IntoElement {
    let cols = snap.cols as usize;
    let mut col = div()
        .flex()
        .flex_col()
        .font_family("JetBrains Mono")
        .text_size(px(13.));
    for row in 0..snap.rows as usize {
        let start = row * cols;
        let line: String = snap.cells[start..start + cols]
            .iter()
            .map(|c| c.ch)
            .collect();
        col = col.child(div().text_color(rgb(0xeeeeee)).child(line));
    }
    col
}
