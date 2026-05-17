pub mod input;
pub mod pty;
pub mod render;
pub mod vt;

pub use vt::{Cell, Snapshot, VtWrapper};

use anyhow::Result;
use std::path::Path;

pub struct Terminal {
    pub pty: pty::PtySession,
    pub vt: VtWrapper,
}

impl Terminal {
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<Self> {
        let pty = pty::PtySession::spawn(cwd, cols, rows)?;
        let vt = VtWrapper::new(cols, rows)?;
        Ok(Self { pty, vt })
    }

    /// Drains any pending PTY output into the VT. Returns whether anything changed.
    pub fn tick(&mut self) -> bool {
        let mut changed = false;
        while let Ok(chunk) = self.pty.output_rx.try_recv() {
            self.vt.feed(&chunk);
            changed = true;
        }
        changed
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.pty.write_input(bytes)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.pty.resize(cols, rows)?;
        self.vt.resize(cols, rows)?;
        Ok(())
    }

    pub fn snapshot(&mut self) -> Result<Snapshot> {
        self.vt.snapshot()
    }
}
