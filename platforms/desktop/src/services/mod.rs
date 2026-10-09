//! Pure Rust desktop domain services container.
//!
//! This module and its children strictly prohibit any dependency on `tauri::*`.

use std::path::PathBuf;
use std::sync::Arc;

pub use sona_core::ports::event::EventEmitterPort;

use crate::integrations::asr::AsrState;
use crate::integrations::audio::AudioState;
use crate::platform::app_config::AppConfigService;
use crate::platform::database::DesktopSqliteState;
use crate::platform::history_repository::{
    HistoryRepositoryState, HistoryService, PreparedBackupImportState,
};
use crate::platform::model_downloads::DownloadState;
use crate::platform::sync::DesktopSyncManager;
use crate::platform::tag_repository::ProjectService;
use crate::platform::task_ledger_repository::TaskLedgerService;

pub mod db_runner;

pub use db_runner::*;

/// Pure Rust container aggregating all desktop domain services.
///
/// Strictly prohibits any dependency on `tauri::*`.
#[derive(Clone)]
pub struct DesktopServices {
    pub sqlite: DesktopSqliteState,
    pub projects: Arc<ProjectService>,
    pub history: Arc<HistoryService>,
    pub config: Arc<AppConfigService>,
    pub sync: Arc<DesktopSyncManager>,
    pub task_ledger: Arc<TaskLedgerService>,
    pub audio: Arc<AudioState>,
    pub asr: Arc<AsrState>,
    pub downloads: Arc<DownloadState>,
    pub emitter: Arc<dyn EventEmitterPort>,
}

#[derive(Default)]
pub struct DesktopServicesBuilder {
    sqlite: Option<DesktopSqliteState>,
    sync_config_path: Option<PathBuf>,
    emitter: Option<Arc<dyn EventEmitterPort>>,
    history_state: Option<HistoryRepositoryState>,
    backup_state: Option<PreparedBackupImportState>,
    audio: Option<Arc<AudioState>>,
    asr: Option<Arc<AsrState>>,
    downloads: Option<Arc<DownloadState>>,
}

impl DesktopServicesBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sqlite(mut self, sqlite: DesktopSqliteState) -> Self {
        self.sqlite = Some(sqlite);
        self
    }

    pub fn sync_config_path(mut self, path: PathBuf) -> Self {
        self.sync_config_path = Some(path);
        self
    }

    pub fn event_emitter(mut self, emitter: Arc<dyn EventEmitterPort>) -> Self {
        self.emitter = Some(emitter);
        self
    }

    pub fn history_state(mut self, state: HistoryRepositoryState) -> Self {
        self.history_state = Some(state);
        self
    }

    pub fn backup_state(mut self, state: PreparedBackupImportState) -> Self {
        self.backup_state = Some(state);
        self
    }

    pub fn audio(mut self, audio: Arc<AudioState>) -> Self {
        self.audio = Some(audio);
        self
    }

    pub fn asr(mut self, asr: Arc<AsrState>) -> Self {
        self.asr = Some(asr);
        self
    }

    pub fn downloads(mut self, downloads: Arc<DownloadState>) -> Self {
        self.downloads = Some(downloads);
        self
    }

    pub fn build(self) -> Result<DesktopServices, String> {
        let sqlite = self
            .sqlite
            .ok_or_else(|| "DesktopSqliteState is required to build DesktopServices".to_string())?;
        let emitter = self
            .emitter
            .ok_or_else(|| "EventEmitterPort is required to build DesktopServices".to_string())?;

        let sync_config_path = self
            .sync_config_path
            .unwrap_or_else(|| PathBuf::from(crate::platform::sync::SYNC_CONFIG_FILE));

        let history_state = self.history_state.unwrap_or_default();
        let backup_state = self.backup_state.unwrap_or_default();

        let projects = Arc::new(ProjectService::new(sqlite.clone()));
        let history = Arc::new(HistoryService::new(
            sqlite.clone(),
            history_state,
            backup_state,
        ));
        let config = Arc::new(AppConfigService::new(sqlite.clone()));
        let sync = Arc::new(DesktopSyncManager::new(sync_config_path, sqlite.clone()));
        let task_ledger = Arc::new(TaskLedgerService::new(sqlite.clone(), Arc::clone(&emitter)));

        let audio = self.audio.unwrap_or_else(|| Arc::new(AudioState::new()));
        let asr = self.asr.unwrap_or_else(|| Arc::new(AsrState::new()));
        let downloads = self
            .downloads
            .unwrap_or_else(|| Arc::new(DownloadState::new()));

        Ok(DesktopServices {
            sqlite,
            projects,
            history,
            config,
            sync,
            task_ledger,
            audio,
            asr,
            downloads,
            emitter,
        })
    }
}

impl DesktopServices {
    pub fn builder() -> DesktopServicesBuilder {
        DesktopServicesBuilder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::event::MockEventEmitter;

    #[tokio::test]
    async fn test_desktop_services_composition_without_tauri() {
        let temp = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let emitter = Arc::new(MockEventEmitter::new());

        let services = DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .sync_config_path(temp.path().join("sync.json"))
            .build()
            .unwrap();

        // Verify individual service calls through DesktopServices facade
        let projects = services.projects.list().await.unwrap();
        assert_eq!(projects.len(), 0);

        let config = services.config.load().unwrap();
        assert_eq!(config, None);

        let sync_status = services.sync.get_status().await.unwrap();
        assert_eq!(
            sync_status.state,
            sona_core::sync::SyncLifecycleState::Disabled
        );

        let ledger_snapshot = services.task_ledger.load_snapshot().await.unwrap();
        assert_eq!(ledger_snapshot.tasks.len(), 0);

        let history_items = services
            .history
            .query_db(|s| s.list_items(Default::default()))
            .await
            .unwrap();
        assert_eq!(history_items.len(), 0);
    }

    #[tokio::test]
    async fn test_desktop_services_hot_reload() {
        use sona_core::project::ProjectCreateInput;

        let temp1 = tempfile::tempdir().unwrap();
        let temp2 = tempfile::tempdir().unwrap();

        let ctx1 = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp1.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx1);
        let emitter = Arc::new(MockEventEmitter::new());

        let services = DesktopServices::builder()
            .sqlite(sqlite)
            .event_emitter(emitter)
            .sync_config_path(temp1.path().join("sync.json"))
            .build()
            .unwrap();

        // 1. Create a project in context 1
        services
            .projects
            .create(ProjectCreateInput {
                name: "Project in Context 1".to_string(),
                description: None,
                icon: None,
                color: None,
                pipeline: None,
            })
            .await
            .unwrap();
        assert_eq!(services.projects.list().await.unwrap().len(), 1);

        // 2. Hot-reload sqlite context to context 2
        let ctx2 = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp2.path()).unwrap());
        services.sqlite.reload(ctx2).unwrap();

        // 3. Verify services dynamically sees context 2 without stale connection or crash
        assert_eq!(services.projects.list().await.unwrap().len(), 0);

        // 4. Create a project in context 2
        services
            .projects
            .create(ProjectCreateInput {
                name: "Project in Context 2".to_string(),
                description: None,
                icon: None,
                color: None,
                pipeline: None,
            })
            .await
            .unwrap();
        assert_eq!(services.projects.list().await.unwrap().len(), 1);
    }
}
