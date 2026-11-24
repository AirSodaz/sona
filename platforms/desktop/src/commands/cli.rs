pub use crate::platform::cli::{
    CliPathEntry, CliPathEntrySource, CliPathState, CliStatus, CompletionInstallState,
    ShellCompletionStatus,
};
use crate::platform::cli::{completion, path_env};

#[tauri::command]
pub async fn cli_get_status() -> Result<CliStatus, String> {
    tokio::task::spawn_blocking(path_env::build_cli_status_sync)
        .await
        .map_err(|e| format!("Failed to inspect CLI PATH status: {e}"))
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cli_set_path_enabled(enabled: bool) -> Result<CliStatus, String> {
    tokio::task::spawn_blocking(move || path_env::set_path_enabled_sync(enabled))
        .await
        .map_err(|e| format!("Failed to update CLI PATH setting: {e}"))??;
    cli_get_status().await
}

#[tauri::command]
pub async fn cli_switch_path_to_current() -> Result<CliStatus, String> {
    tokio::task::spawn_blocking(path_env::switch_path_to_current_sync)
        .await
        .map_err(|e| format!("Failed to switch CLI PATH: {e}"))??;
    cli_get_status().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cli_remove_path_entry(path_dir: String) -> Result<CliStatus, String> {
    tokio::task::spawn_blocking(move || path_env::remove_path_entry_sync(path_dir))
        .await
        .map_err(|e| format!("Failed to remove PATH entry: {e}"))??;
    cli_get_status().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cli_get_completion_script(shell: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || completion::get_completion_script_sync(&shell))
        .await
        .map_err(|e| format!("Failed to generate shell completion: {e}"))?
}

#[tauri::command]
pub async fn cli_get_completion_statuses() -> Result<Vec<ShellCompletionStatus>, String> {
    tokio::task::spawn_blocking(completion::get_all_completion_statuses_sync)
        .await
        .map_err(|e| format!("Failed to inspect shell completions: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cli_install_completion(shell: String) -> Result<ShellCompletionStatus, String> {
    tokio::task::spawn_blocking(move || completion::install_completion_sync(&shell))
        .await
        .map_err(|e| format!("Failed to install shell completion: {e}"))?
}

#[tauri::command(rename_all = "camelCase")]
pub async fn cli_uninstall_completion(shell: String) -> Result<ShellCompletionStatus, String> {
    tokio::task::spawn_blocking(move || completion::uninstall_completion_sync(&shell))
        .await
        .map_err(|e| format!("Failed to uninstall shell completion: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cli_status() {
        let status = cli_get_status().await.unwrap();
        assert!(!status.version.is_empty());
        assert!(!status.platform.is_empty());
        if status.installed {
            assert!(status.binary_path.is_some());
        }
    }
}
