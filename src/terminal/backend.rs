use crate::terminal::colors::{ANSI_256, DEFAULT_BG, DEFAULT_FG, resolve_named, rgb_to_u32};
use crate::terminal::listener::SwrmListener;
use crate::terminal::snapshot::{Cell, CellFlags, Snapshot};
use alacritty_terminal::event::{Event as AlacEvent, WindowSize};
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg, State};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags as AlacFlags;
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::tty;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, Processor, Rgb, StdSyncHandler};
use anyhow::{Context, Result};
use futures::channel::mpsc::UnboundedReceiver;
use std::borrow::Cow;
use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;

#[derive(Clone, Copy)]
pub struct TermSize {
    pub columns: usize,
    pub screen_lines: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        // 10_000 lines of scrollback history above the visible screen.
        self.screen_lines + 10_000
    }
    fn screen_lines(&self) -> usize {
        self.screen_lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

impl From<TermSize> for WindowSize {
    fn from(s: TermSize) -> Self {
        WindowSize {
            num_lines: s.screen_lines as u16,
            num_cols: s.columns as u16,
            cell_width: 0,
            cell_height: 0,
        }
    }
}

pub struct Backend {
    pub(crate) term: Arc<FairMutex<Term<SwrmListener>>>,
    pub(crate) tx: Option<EventLoopSender>,
    _join: Option<JoinHandle<(EventLoop<tty::Pty, SwrmListener>, State)>>,
    size: TermSize,
}

impl Backend {
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<(Self, UnboundedReceiver<AlacEvent>)> {
        Self::spawn_with_args(cwd, vec![], cols, rows)
    }

    /// Spawn a PTY that runs `$SHELL -c <command>`. `command` is a shell-eval'd
    /// string, NOT an argv — quoting, expansion, pipes, and aliases all apply.
    /// Use this to launch a tab into a specific agent invocation.
    pub fn spawn_command(
        cwd: &Path,
        command: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(Self, UnboundedReceiver<AlacEvent>)> {
        Self::spawn_with_args(cwd, vec!["-c".into(), command.to_string()], cols, rows)
    }

    pub fn headless(cols: u16, rows: u16) -> (Self, UnboundedReceiver<AlacEvent>) {
        let size = TermSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let (listener, events_rx) = SwrmListener::pair();
        let term = Term::new(Config::default(), &size, listener);
        let term = Arc::new(FairMutex::new(term));
        (
            Self {
                term,
                tx: None,
                _join: None,
                size,
            },
            events_rx,
        )
    }

    pub fn size(&self) -> TermSize {
        self.size
    }

    pub fn mode(&self) -> TermMode {
        *self.term.lock().mode()
    }

    pub fn write_input(&self, bytes: &[u8]) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Input(Cow::Owned(bytes.to_vec())));
        }
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        let new_size = TermSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        if new_size.columns == self.size.columns && new_size.screen_lines == self.size.screen_lines
        {
            return Ok(());
        }
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Resize(new_size.into()));
        }
        self.term.lock().resize(new_size);
        self.size = new_size;
        Ok(())
    }

    pub fn scroll(&self, delta_lines: i32) {
        self.term.lock().scroll_display(Scroll::Delta(delta_lines));
    }

    pub fn shutdown(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Msg::Shutdown);
        }
    }

    /// Feed raw bytes through a fresh vte parser into the Term. SYNCHRONOUS and
    /// for headless / test use only. A new `Processor` is created per call, so
    /// state does NOT persist across invocations — callers must pass complete
    /// VT sequences in a single call. Production byte flow goes through the
    /// alacritty `EventLoop` thread, which holds a long-lived parser.
    pub fn feed_bytes(&self, bytes: &[u8]) {
        let mut processor: Processor<StdSyncHandler> = Processor::new();
        let mut term = self.term.lock();
        processor.advance(&mut *term, bytes);
    }

    pub fn snapshot(&self) -> Snapshot {
        let term = self.term.lock();
        let content = term.renderable_content();
        let cols = self.size.columns as u16;
        let rows = self.size.screen_lines as u16;

        let capacity = cols as usize * rows as usize;
        let mut cells: Vec<Cell> = vec![
            Cell {
                ch: ' ',
                fg: rgb_to_u32(DEFAULT_FG),
                bg: rgb_to_u32(DEFAULT_BG),
                flags: CellFlags::empty(),
            };
            capacity
        ];

        let display_offset = content.display_offset as i32;

        for indexed in content.display_iter {
            let row = indexed.point.line.0 + display_offset;
            if row < 0 {
                continue;
            }
            let row_visible = row as usize;
            if row_visible >= rows as usize {
                continue;
            }
            let col = indexed.point.column.0;
            if col >= cols as usize {
                continue;
            }
            let idx = row_visible * cols as usize + col;

            let alac_flags = indexed.cell.flags;
            let mut flags = CellFlags::empty();
            if alac_flags.contains(AlacFlags::BOLD) {
                flags |= CellFlags::BOLD;
            }
            if alac_flags.contains(AlacFlags::ITALIC) {
                flags |= CellFlags::ITALIC;
            }
            if alac_flags.intersects(AlacFlags::ALL_UNDERLINES) {
                flags |= CellFlags::UNDERLINE;
            }
            if alac_flags.contains(AlacFlags::STRIKEOUT) {
                flags |= CellFlags::STRIKEOUT;
            }
            if alac_flags.contains(AlacFlags::INVERSE) {
                flags |= CellFlags::INVERSE;
            }
            if alac_flags.contains(AlacFlags::DIM) {
                flags |= CellFlags::DIM;
            }

            let mut fg = resolve_color(indexed.cell.fg);
            let mut bg = resolve_color(indexed.cell.bg);
            if flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            cells[idx] = Cell {
                ch: indexed.cell.c,
                fg: rgb_to_u32(fg),
                bg: rgb_to_u32(bg),
                flags,
            };
        }

        let cursor_point = content.cursor.point;
        let cursor_row = (cursor_point.line.0 + display_offset).max(0) as u16;
        let cursor_col = cursor_point.column.0 as u16;
        let cursor_visible = term.mode().contains(TermMode::SHOW_CURSOR);

        Snapshot {
            cols,
            rows,
            cells,
            cursor_row,
            cursor_col,
            cursor_visible,
        }
    }

    fn spawn_with_args(
        cwd: &Path,
        shell_args: Vec<String>,
        cols: u16,
        rows: u16,
    ) -> Result<(Self, UnboundedReceiver<AlacEvent>)> {
        let size = TermSize {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let (listener, events_rx) = SwrmListener::pair();
        let term = Term::new(Config::default(), &size, listener.clone());
        let term = Arc::new(FairMutex::new(term));

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut tty_opts = tty::Options::default();
        tty_opts.shell = Some(tty::Shell::new(shell, shell_args));
        tty_opts.working_directory = Some(cwd.to_path_buf());
        tty_opts.drain_on_exit = true;
        tty_opts.env = child_env();

        let pty = tty::new(&tty_opts, size.into(), 0).context("open pty + fork shell")?;

        let event_loop = EventLoop::new(term.clone(), listener, pty, true, false)
            .context("build alacritty event loop")?;

        let tx = event_loop.channel();
        let _join = Some(event_loop.spawn());

        Ok((
            Self {
                term,
                tx: Some(tx),
                _join,
                size,
            },
            events_rx,
        ))
    }
}

fn resolve_color(c: AnsiColor) -> Rgb {
    match c {
        AnsiColor::Named(n) => resolve_named(n),
        AnsiColor::Spec(rgb) => rgb,
        AnsiColor::Indexed(i) => ANSI_256[i as usize],
    }
}

/// Environment variables forced onto the child shell. We override `TERM` so the
/// shell uses OSC title sequences (which alacritty's vte parser handles) rather
/// than the `screen`/`tmux` `ESC k … ESC \` form (which it doesn't — the `k`
/// terminates the escape, dropping the parser back to ground and printing the
/// payload as literal text). Without this, launching swrm from a tmux session
/// inherits `TERM=tmux-256color`, oh-my-zsh's preexec hook sets the tab title
/// with `\ek<cmd>\e\\`, and the command name leaks onto the next line right
/// before its output (e.g. `lsCargo.lock` instead of `Cargo.lock`).
fn child_env() -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    env.insert("TERM".into(), "xterm-256color".into());
    env.insert("COLORTERM".into(), "truecolor".into());
    env
}
