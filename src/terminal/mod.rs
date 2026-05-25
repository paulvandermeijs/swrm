pub mod backend;
pub mod colors;
pub mod input;
pub mod listener;
pub mod render;
pub mod snapshot;

pub(crate) use backend::Backend;
pub use snapshot::{Cell, CellFlags, Snapshot};

use alacritty_terminal::event::Event as AlacEvent;
use alacritty_terminal::term::TermMode;
use anyhow::Result;
use futures::channel::mpsc::UnboundedReceiver;
use std::path::Path;

pub struct Terminal {
    backend: Backend,
    events: Option<UnboundedReceiver<AlacEvent>>,
}

impl Terminal {
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<Self> {
        let (backend, events) = Backend::spawn(cwd, cols, rows)?;
        Ok(Self {
            backend,
            events: Some(events),
        })
    }

    pub fn spawn_command(cwd: &Path, command: &str, cols: u16, rows: u16) -> Result<Self> {
        let (backend, events) = Backend::spawn_command(cwd, command, cols, rows)?;
        Ok(Self {
            backend,
            events: Some(events),
        })
    }

    pub fn headless(cols: u16, rows: u16) -> Self {
        let (backend, events) = Backend::headless(cols, rows);
        Self {
            backend,
            events: Some(events),
        }
    }

    pub fn take_events(&mut self) -> Option<UnboundedReceiver<AlacEvent>> {
        self.events.take()
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.backend.write_input(bytes);
        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.backend.resize(cols, rows)
    }

    pub fn scroll(&self, delta_lines: i32) {
        self.backend.scroll(delta_lines);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.backend.snapshot()
    }

    pub fn mode(&self) -> TermMode {
        self.backend.mode()
    }

    pub fn feed_bytes(&self, bytes: &[u8]) {
        self.backend.feed_bytes(bytes);
    }
}
