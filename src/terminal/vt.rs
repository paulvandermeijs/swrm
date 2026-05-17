use anyhow::Result;
use libghostty_vt::{
    RenderState, Terminal, TerminalOptions,
    render::{CellIterator, RowIterator},
};

/// A single terminal cell with its character and colors.
pub struct Cell {
    pub ch: char,
    /// Foreground color packed as 0x00RRGGBB.
    pub fg: u32,
    /// Background color packed as 0x00RRGGBB.
    pub bg: u32,
}

/// A snapshot of the visible terminal screen.
pub struct Snapshot {
    pub cols: u16,
    pub rows: u16,
    /// Row-major grid; `len == cols * rows`.
    pub cells: Vec<Cell>,
    pub cursor_row: u16,
    pub cursor_col: u16,
}

/// Thin facade around a libghostty-vt terminal and its render state.
pub struct VtWrapper {
    pub terminal: Terminal<'static, 'static>,
    pub render_state: RenderState<'static>,
    row_iter: RowIterator<'static>,
    cell_iter: CellIterator<'static>,
}

impl VtWrapper {
    pub fn new(cols: u16, rows: u16) -> Result<Self> {
        let terminal = Terminal::new(TerminalOptions {
            cols,
            rows,
            max_scrollback: 10_000,
        })?;
        let render_state = RenderState::new()?;
        let row_iter = RowIterator::new()?;
        let cell_iter = CellIterator::new()?;
        Ok(Self {
            terminal,
            render_state,
            row_iter,
            cell_iter,
        })
    }

    /// Feed raw VT bytes into the terminal.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.terminal.vt_write(bytes);
    }

    /// Resize the terminal grid. Pixel dimensions default to 0 (unknown).
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.terminal.resize(cols, rows, 0, 0)?;
        Ok(())
    }

    /// Capture the current visible screen as a flat `Snapshot`.
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let snapshot = self.render_state.update(&self.terminal)?;

        let cols = snapshot.cols()?;
        let rows = snapshot.rows()?;

        let default_fg = 0x00ee_eeee_u32;
        let default_bg = 0x0000_0000_u32;

        let capacity = cols as usize * rows as usize;
        let mut cells: Vec<Cell> = Vec::with_capacity(capacity);

        let mut row_iteration = self.row_iter.update(&snapshot)?;
        while let Some(row) = row_iteration.next() {
            let mut cell_iteration = self.cell_iter.update(row)?;
            let mut col_count = 0u16;
            while let Some(cell) = cell_iteration.next() {
                let graphemes = cell.graphemes()?;
                let ch = graphemes.into_iter().next().unwrap_or(' ');

                let fg = cell
                    .fg_color()?
                    .map(|c| rgb_to_u32(c.r, c.g, c.b))
                    .unwrap_or(default_fg);
                let bg = cell
                    .bg_color()?
                    .map(|c| rgb_to_u32(c.r, c.g, c.b))
                    .unwrap_or(default_bg);

                cells.push(Cell { ch, fg, bg });
                col_count += 1;
            }
            // Pad any short rows to the full column count.
            while col_count < cols {
                cells.push(Cell {
                    ch: ' ',
                    fg: default_fg,
                    bg: default_bg,
                });
                col_count += 1;
            }
        }

        // Read cursor position directly from the terminal (avoids snapshot
        // borrow issues; cursor_x/cursor_y are always in active-area coords).
        let cursor_col = self.terminal.cursor_x()?;
        let cursor_row = self.terminal.cursor_y()?;

        drop(row_iteration);

        Ok(Snapshot {
            cols,
            rows,
            cells,
            cursor_row,
            cursor_col,
        })
    }
}

#[inline]
fn rgb_to_u32(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}
