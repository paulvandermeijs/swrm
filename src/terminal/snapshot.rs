use bitflags::bitflags;

bitflags! {
    /// Per-cell display attributes. Subset of alacritty's `term::cell::Flags`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CellFlags: u8 {
        const BOLD          = 0b0000_0001;
        const ITALIC        = 0b0000_0010;
        const UNDERLINE     = 0b0000_0100;
        const STRIKEOUT     = 0b0000_1000;
        const INVERSE       = 0b0001_0000;
        const DIM           = 0b0010_0000;
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub ch: char,
    /// 0x00RRGGBB
    pub fg: u32,
    /// 0x00RRGGBB
    pub bg: u32,
    pub flags: CellFlags,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cols: u16,
    pub rows: u16,
    /// Row-major; `len == cols * rows`.
    pub cells: Vec<Cell>,
    pub cursor_row: u16,
    pub cursor_col: u16,
    /// True if the cursor should be drawn (i.e. terminal is not in DECTCEM-hidden mode).
    pub cursor_visible: bool,
}

impl Snapshot {
    pub fn cell_at(&self, row: u16, col: u16) -> Option<&Cell> {
        let idx = row as usize * self.cols as usize + col as usize;
        self.cells.get(idx)
    }
}
