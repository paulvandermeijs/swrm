pub mod activity;
pub mod event;
pub mod server;
pub mod settings_file;
pub mod store;

pub use activity::{extract_activity, format_activity};
pub use event::AgentStatus;
pub use server::{HookEvent, start_server};
pub use settings_file::{
    build_claude_settings_json, has_placeholder, substitute_placeholder, temp_settings_dir,
    write_settings_file,
};
pub use store::{AgentStatusEvent, AgentStatusStore};
