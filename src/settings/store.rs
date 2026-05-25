use super::{Agent, persistence};
use gpui::{Context, EventEmitter};

#[derive(Clone, Debug)]
pub enum SettingsEvent {
    Changed,
}

#[derive(Clone, Copy, Debug)]
pub enum MoveDir {
    Up,
    Down,
}

pub struct SettingsStore {
    settings: super::AppSettings,
}

impl SettingsStore {
    pub fn load(_cx: &mut Context<Self>) -> Self {
        let settings = persistence::load().unwrap_or_else(|err| {
            tracing::warn!(?err, "failed to load settings, starting empty");
            super::AppSettings::default()
        });
        Self { settings }
    }

    pub fn agents(&self) -> &[Agent] {
        &self.settings.agents
    }

    pub fn add_agent(&mut self, cx: &mut Context<Self>) -> String {
        let id = next_id(&self.settings.agents);
        self.settings.agents.push(Agent {
            id: id.clone(),
            name: String::new(),
            command: String::new(),
        });
        self.persist();
        cx.emit(SettingsEvent::Changed);
        cx.notify();
        id
    }

    pub fn update_agent(
        &mut self,
        id: &str,
        name: String,
        command: String,
        cx: &mut Context<Self>,
    ) {
        let Some(agent) = self.settings.agents.iter_mut().find(|a| a.id == id) else {
            return;
        };
        if agent.name == name && agent.command == command {
            return;
        }
        agent.name = name;
        agent.command = command;
        self.persist();
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }

    pub fn remove_agent(&mut self, id: &str, cx: &mut Context<Self>) {
        let before = self.settings.agents.len();
        self.settings.agents.retain(|a| a.id != id);
        if self.settings.agents.len() != before {
            self.persist();
            cx.emit(SettingsEvent::Changed);
            cx.notify();
        }
    }

    pub fn move_agent(&mut self, id: &str, dir: MoveDir, cx: &mut Context<Self>) {
        let Some(idx) = self.settings.agents.iter().position(|a| a.id == id) else {
            return;
        };
        let target = match dir {
            MoveDir::Up if idx > 0 => idx - 1,
            MoveDir::Down if idx + 1 < self.settings.agents.len() => idx + 1,
            _ => return,
        };
        self.settings.agents.swap(idx, target);
        self.persist();
        cx.emit(SettingsEvent::Changed);
        cx.notify();
    }

    fn persist(&self) {
        if let Err(err) = persistence::save(&self.settings) {
            tracing::error!(?err, "failed to persist settings");
        }
    }
}

impl EventEmitter<SettingsEvent> for SettingsStore {}

pub fn next_id(agents: &[Agent]) -> String {
    let max = agents
        .iter()
        .filter_map(|a| {
            a.id.strip_prefix("agent-")
                .and_then(|n| n.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    format!("agent-{}", max + 1)
}
