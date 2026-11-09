pub mod agent_control;
pub mod audio_playback;
pub mod auto_launch;
pub mod hotkey;
pub mod models;
pub mod opener;
pub mod repositories;
pub mod single_instance;
pub mod storage;
pub mod system;
pub mod tray;
pub mod window_decorations;

// Re-exports for backwards compatibility and clean public API
pub use auto_launch::{
    AutoLaunchManager, AutoLaunchStatus, MockAutoLaunch, clear_mock_manager, disable_auto_launch,
    enable_auto_launch, get_auto_launch_status, is_auto_launch_enabled, set_mock_manager,
};
pub use opener::{open_path, open_path_async, open_url, open_url_async};
pub use single_instance::SingleInstanceGuard;

pub use models::downloads as model_downloads;
pub use models::hardware;
pub use models::media_detector;
pub use models::preset as preset_models;
pub use models::speaker_processing;

pub use storage::audio as audio_storage;
pub use storage::database;
pub use storage::file as file_storage;
pub use storage::location as storage_location;
pub use storage::usage as storage_usage;

pub use system::archive;
pub use system::audio as system_audio;
pub use system::blocking;
pub use system::console as startup_console;
pub use system::dialog as startup_dialog;
pub use system::env as startup_env;
pub use system::event;
pub use system::host;
pub use system::paths;
pub use system::status as runtime_status;
pub use system::time;

pub use repositories::api_server_config;
pub use repositories::api_server_runtime;
pub use repositories::app_config;
pub use repositories::automation as automation_repository;
pub use repositories::automation_runtime;
pub use repositories::dashboard;
pub use repositories::diagnostics;
pub use repositories::history as history_repository;
pub use repositories::llm_usage;
pub use repositories::project as project_repository;
pub use repositories::recovery as recovery_repository;
pub use repositories::sync;
pub use repositories::sync_secret_store;
pub use repositories::task_ledger as task_ledger_repository;
