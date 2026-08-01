use crate::config::Config;
pub use lemurclaw_rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use lemurclaw_rollout::Cursor;
pub use lemurclaw_rollout::INTERACTIVE_SESSION_SOURCES;
pub use lemurclaw_rollout::RolloutRecorder;
pub use lemurclaw_rollout::RolloutRecorderParams;
pub use lemurclaw_rollout::SESSIONS_SUBDIR;
pub use lemurclaw_rollout::SessionMeta;
pub use lemurclaw_rollout::SortDirection;
pub use lemurclaw_rollout::ThreadItem;
pub use lemurclaw_rollout::ThreadSortKey;
pub use lemurclaw_rollout::ThreadsPage;
pub use lemurclaw_rollout::append_thread_name;
pub use lemurclaw_rollout::find_archived_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use lemurclaw_rollout::find_conversation_path_by_id_str;
pub use lemurclaw_rollout::find_thread_meta_by_name_str;
pub use lemurclaw_rollout::find_thread_name_by_id;
pub use lemurclaw_rollout::find_thread_names_by_ids;
pub use lemurclaw_rollout::find_thread_path_by_id_str;
pub use lemurclaw_rollout::parse_cursor;
pub use lemurclaw_rollout::read_head_for_summary;
pub use lemurclaw_rollout::read_session_meta_line;
pub use lemurclaw_rollout::rollout_date_parts;

impl lemurclaw_rollout::RolloutConfigView for Config {
    fn codex_home(&self) -> &std::path::Path {
        self.codex_home.as_path()
    }

    fn sqlite_config(&self) -> &lemurclaw_state::SqliteConfig {
        self.sqlite_config()
    }

    fn cwd(&self) -> &std::path::Path {
        self.cwd.as_path()
    }

    fn model_provider_id(&self) -> &str {
        self.model_provider_id.as_str()
    }

    fn generate_memories(&self) -> bool {
        self.memories.generate_memories
    }
}

pub(crate) mod list {
    pub use lemurclaw_rollout::find_thread_path_by_id_str;
}

#[cfg(test)]
pub(crate) mod recorder {
    pub use lemurclaw_rollout::RolloutRecorder;
}

pub(crate) use crate::session_rollout_init_error::map_session_init_error;

pub(crate) mod truncation {
    pub(crate) use crate::thread_rollout_truncation::*;
}
