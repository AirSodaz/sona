pub mod asr_config;
pub mod batch;
pub mod error;
pub mod history;
pub mod models;
pub mod projects;
pub mod session;
pub mod settings;
pub mod sync;
pub mod transcript;
pub mod types;

#[cfg(test)]
mod tests;

pub use asr_config::*;
pub use batch::*;
pub use error::*;
pub use history::*;
pub use models::*;
pub use projects::*;
pub use session::*;
pub use settings::*;
pub use sync::*;
pub use transcript::*;
pub use types::*;

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::services::DesktopServices;
use sona_core::history::{HistorySummaryPayload, TranscriptSnapshotMetadata};
use sona_core::project::ProjectRecord;
use sona_core::sync::SyncConflictSummary;

#[derive(Clone)]
pub struct AgentControlFacade {
    pub(crate) services: DesktopServices,
    pub(crate) app_handle: Option<tauri::AppHandle>,
    pub(crate) active_session: Arc<Mutex<Option<ActiveRecordingSession>>>,
    pub(crate) recording_op_lock: Arc<Mutex<()>>,
}

impl AgentControlFacade {
    pub fn new(services: DesktopServices, app_handle: Option<tauri::AppHandle>) -> Self {
        let active_session = services.recording_session.handle();
        Self {
            services,
            app_handle,
            active_session,
            recording_op_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn get_client_state(&self) -> Result<ClientStateResult, String> {
        let (is_recording, is_paused) = {
            let session_guard = self.active_session.lock().await;
            if let Some(session) = session_guard.as_ref() {
                (true, session.is_paused)
            } else {
                (self.services.audio.has_active_captures(), false)
            }
        };
        let active_project_id = self
            .services
            .projects
            .get_active_project_id()
            .await
            .ok()
            .flatten();
        Ok(ClientStateResult {
            online: true,
            is_recording,
            is_paused,
            active_project_id,
        })
    }

    pub fn focus_window(&self) -> Result<bool, String> {
        if let Some(app) = &self.app_handle {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window(crate::app::window::MAIN_WINDOW_LABEL) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn start_recording(
        &self,
        req: StartRecordingRequest,
    ) -> Result<StartRecordingResult, String> {
        session::start_recording(
            &self.services,
            self.app_handle.as_ref(),
            &self.active_session,
            &self.recording_op_lock,
            req,
        )
        .await
    }

    pub async fn stop_recording(
        &self,
        req: StopRecordingRequest,
    ) -> Result<StopRecordingResult, String> {
        session::stop_recording(
            &self.services,
            self.app_handle.as_ref(),
            &self.active_session,
            &self.recording_op_lock,
            req,
        )
        .await
    }

    pub async fn pause_recording(&self) -> Result<bool, String> {
        session::pause_recording(
            &self.services,
            &self.active_session,
            &self.recording_op_lock,
        )
        .await
    }

    pub async fn resume_recording(&self) -> Result<bool, String> {
        session::resume_recording(
            &self.services,
            &self.active_session,
            &self.recording_op_lock,
        )
        .await
    }

    pub async fn list_audio_devices(&self) -> Result<AudioDevicesResult, String> {
        session::list_audio_devices().await
    }

    pub async fn read_transcript(
        &self,
        history_id: String,
    ) -> Result<ReadTranscriptResult, String> {
        transcript::read_transcript(&self.services, history_id).await
    }

    pub async fn edit_transcript(
        &self,
        req: EditTranscriptRequest,
    ) -> Result<EditTranscriptResult, String> {
        transcript::edit_transcript(&self.services, req).await
    }

    pub async fn patch_segments(
        &self,
        req: PatchSegmentsRequest,
    ) -> Result<PatchSegmentsResult, String> {
        transcript::patch_segments(&self.services, req).await
    }

    pub async fn update_translations(
        &self,
        req: UpdateTranslationsRequest,
    ) -> Result<UpdateTranslationsResult, String> {
        transcript::update_translations(&self.services, req).await
    }

    pub async fn list_transcript_snapshots(
        &self,
        history_id: String,
    ) -> Result<Vec<TranscriptSnapshotMetadata>, String> {
        transcript::list_transcript_snapshots(&self.services, history_id).await
    }

    pub async fn revert_transcript_snapshot(
        &self,
        history_id: String,
        snapshot_id: String,
    ) -> Result<RevertTranscriptResult, String> {
        transcript::revert_transcript_snapshot(&self.services, history_id, snapshot_id).await
    }

    pub async fn query_history(
        &self,
        req: QueryHistoryRequest,
    ) -> Result<Vec<HistoryItemSummary>, String> {
        history::query_history(&self.services, req).await
    }

    pub async fn delete_history(
        &self,
        history_id: String,
        permanent: bool,
    ) -> Result<bool, String> {
        history::delete_history(&self.services, history_id, permanent).await
    }

    pub async fn update_history_meta(
        &self,
        history_id: String,
        req: UpdateHistoryMetaRequest,
    ) -> Result<bool, String> {
        history::update_history_meta(&self.services, history_id, req).await
    }

    pub async fn query_trash(
        &self,
        req: QueryTrashRequest,
    ) -> Result<Vec<HistoryItemSummary>, String> {
        history::query_trash(&self.services, req).await
    }

    pub async fn restore_history(&self, history_id: String) -> Result<bool, String> {
        history::restore_history(&self.services, history_id).await
    }

    pub async fn save_summary(&self, req: SaveSummaryRequest) -> Result<bool, String> {
        history::save_summary(&self.services, req).await
    }

    pub async fn delete_summary(&self, history_id: String) -> Result<bool, String> {
        history::delete_summary(&self.services, history_id).await
    }

    pub async fn load_summary(
        &self,
        history_id: String,
    ) -> Result<Option<HistorySummaryPayload>, String> {
        history::load_summary(&self.services, history_id).await
    }

    pub async fn transcribe_file(
        &self,
        req: TranscribeFileRequest,
    ) -> Result<TranscribeFileResult, String> {
        batch::transcribe_file(&self.services, req).await
    }

    pub async fn cancel_batch_task(&self, instance_id: String) -> Result<bool, String> {
        batch::cancel_batch_task(&self.services, instance_id).await
    }

    pub async fn export_transcript(
        &self,
        req: ExportTranscriptRequest,
    ) -> Result<ExportTranscriptResult, String> {
        batch::export_transcript(&self.services, req).await
    }

    pub async fn list_projects(&self) -> Result<Vec<ProjectRecord>, String> {
        projects::list_projects(&self.services).await
    }

    pub async fn set_active_project(&self, project_id: Option<String>) -> Result<bool, String> {
        projects::set_active_project(&self.services, project_id).await
    }

    pub async fn create_project(&self, req: CreateProjectRequest) -> Result<ProjectRecord, String> {
        projects::create_project(&self.services, req).await
    }

    pub async fn update_project(
        &self,
        project_id: String,
        req: UpdateProjectRequest,
    ) -> Result<Option<ProjectRecord>, String> {
        projects::update_project(&self.services, project_id, req).await
    }

    pub async fn delete_project(
        &self,
        project_id: String,
        cascade_action: Option<String>,
    ) -> Result<bool, String> {
        projects::delete_project(&self.services, project_id, cascade_action).await
    }

    pub fn get_settings(&self, key: Option<String>) -> Result<serde_json::Value, String> {
        settings::get_settings(&self.services, key)
    }

    pub fn update_setting(&self, key: String, value: serde_json::Value) -> Result<bool, String> {
        settings::update_setting(&self.services, key, value)
    }

    pub async fn get_model_catalog(&self) -> Result<serde_json::Value, String> {
        models::get_model_catalog(&self.services, self.app_handle.as_ref()).await
    }

    pub async fn download_preset_model(
        &self,
        req: DownloadPresetModelRequest,
    ) -> Result<String, String> {
        models::download_preset_model(&self.services, self.app_handle.as_ref(), req).await
    }

    pub async fn cancel_download(&self, download_id: String) -> Result<bool, String> {
        models::cancel_download(&self.services, download_id).await
    }

    pub async fn delete_preset_model(&self, model_id: String) -> Result<bool, String> {
        models::delete_preset_model(self.app_handle.as_ref(), model_id).await
    }

    pub async fn get_sync_status(&self) -> Result<serde_json::Value, String> {
        sync::get_sync_status(&self.services).await
    }

    pub async fn trigger_sync(&self) -> Result<serde_json::Value, String> {
        sync::trigger_sync(&self.services).await
    }

    pub async fn list_sync_conflicts(&self) -> Result<Vec<SyncConflictSummary>, String> {
        sync::list_sync_conflicts(&self.services).await
    }

    pub async fn resolve_sync_conflict(
        &self,
        conflict_id: String,
        resolution: String,
    ) -> Result<bool, String> {
        sync::resolve_sync_conflict(&self.services, conflict_id, resolution).await
    }
}
