use std::time::{Duration, Instant};
use swrm::terminal::Terminal;

/// Verifies that `Terminal::spawn_command` runs the given string through
/// `$SHELL -c` and emits its stdout to the PTY. Mirrors the polling pattern
/// used by `child_shell_sees_xterm_256color_term` in `vt_smoke.rs`.
#[test]
fn agent_command_runs_through_shell() {
    let cwd = std::env::temp_dir();
    let mut term = Terminal::spawn_command(&cwd, "printf 'AGENT=[%s]' yes", 80, 6)
        .expect("spawn agent command");
    let _ = term.take_events();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snap = term.snapshot();
        let grid: String = snap.cells.iter().map(|c| c.ch).collect();
        if grid.contains("AGENT=[yes]") {
            return;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for command output; grid = {grid:?}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
