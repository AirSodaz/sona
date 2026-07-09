pub async fn run_blocking_asr_task<F, R>(task: F) -> Result<R, String>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|error| error.to_string())
}

pub fn spawn_blocking_asr_task<F>(task: F)
where
    F: FnOnce() + Send + 'static,
{
    drop(tauri::async_runtime::spawn_blocking(task));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn run_blocking_asr_task_returns_closure_result() {
        let value = run_blocking_asr_task(|| 42).await.unwrap();

        assert_eq!(value, 42);
    }

    #[tokio::test]
    async fn spawn_blocking_asr_task_runs_detached_work() {
        let (tx, rx) = tokio::sync::oneshot::channel();

        spawn_blocking_asr_task(move || {
            let _ = tx.send("ran");
        });

        let value = tokio::time::timeout(Duration::from_secs(2), rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(value, "ran");
    }
}
