use std::future::Future;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

use crate::{CliError, CliResult};

static CLI_RUNTIME: LazyLock<Result<Runtime, String>> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
});

/// Returns a reference to the shared CLI multi-thread Tokio runtime.
pub(crate) fn get_or_init_runtime() -> Result<&'static Runtime, CliError> {
    match &*CLI_RUNTIME {
        Ok(runtime) => Ok(runtime),
        Err(error) => Err(CliError::Io(format!(
            "Failed to create async runtime: {error}"
        ))),
    }
}

/// Executes an asynchronous future to completion on the unified CLI Tokio runtime.
///
/// If a Tokio runtime is already active on the current thread (e.g., in unit/integration tests
/// or embedded harnesses), this enters the existing runtime safely via `block_in_place`.
/// Otherwise, it initializes and blocks on the unified multi-threaded CLI runtime.
pub(crate) fn block_on<F>(future: F) -> CliResult<F::Output>
where
    F: Future + Send,
    F::Output: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                Ok(tokio::task::block_in_place(|| handle.block_on(future)))
            }
            _ => std::thread::scope(|s| {
                s.spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| {
                            CliError::Io(format!("Failed to create async runtime: {e}"))
                        })?;
                    Ok(rt.block_on(future))
                })
                .join()
                .unwrap_or_else(|_| Err(CliError::Other("Async task panicked".to_string())))
            }),
        },
        Err(_) => {
            let runtime = get_or_init_runtime()?;
            Ok(runtime.block_on(future))
        }
    }
}
