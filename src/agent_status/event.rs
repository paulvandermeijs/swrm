/// Lifecycle status reported by an agent's hooks. Wire strings match
/// agent-status / Claude Code conventions (`notify`, `done`, `working`,
/// `idle`) so they can be reused by other agents later. Unknown wire
/// values are dropped at the parsing boundary (`from_wire` returns
/// `None`) rather than carried through as an `Unknown` variant — the
/// current set is sufficient for the indicator and a future agent that
/// needs a new value should add a variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentStatus {
    Notify,
    Done,
    Working,
    Idle,
}

impl AgentStatus {
    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "notify" => Some(Self::Notify),
            "done" => Some(Self::Done),
            "working" => Some(Self::Working),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }

    /// Higher = more attention-worthy. Used to fold multiple tabs'
    /// statuses into a single workspace-level indicator.
    pub fn priority(self) -> u8 {
        match self {
            Self::Notify => 4,
            Self::Done => 3,
            Self::Working => 2,
            Self::Idle => 1,
        }
    }
}
