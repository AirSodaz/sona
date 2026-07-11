#[cfg(target_os = "windows")]
pub fn fix_console(show_new_console: bool) {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;

    unsafe {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn AllocConsole() -> i32;
            fn AttachConsole(dwProcessId: u32) -> i32;
            fn SetStdHandle(nStdHandle: u32, hHandle: *mut std::ffi::c_void) -> i32;
            fn GetLastError() -> u32;
        }

        const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
        const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
        const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
        const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;

        let mut has_console = false;

        // Try to attach to parent console first when the desktop binary is
        // launched from a terminal and we want logs to remain visible there.
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            has_console = true;
        } else if show_new_console {
            // Allocate a console to initialize stdout/stderr handles in the C runtime
            // only if we explicitly want to show one without a parent console.
            if AllocConsole() != 0 {
                has_console = true;
            }
        }

        if has_console {
            // Redirect stdout and stderr
            match OpenOptions::new().write(true).open("CONOUT$") {
                Ok(conout) => {
                    let handle = conout.as_raw_handle();
                    if SetStdHandle(STD_OUTPUT_HANDLE, handle) == 0 {
                        eprintln!(
                            "[debug] Failed to set STD_OUTPUT_HANDLE: GetLastError() = {}",
                            GetLastError()
                        );
                    }
                    if SetStdHandle(STD_ERROR_HANDLE, handle) == 0 {
                        eprintln!(
                            "[debug] Failed to set STD_ERROR_HANDLE: GetLastError() = {}",
                            GetLastError()
                        );
                    }
                    std::mem::forget(conout); // Leak handle so it stays open for the lifetime of the process
                }
                Err(e) => {
                    eprintln!("[debug] Failed to open CONOUT$: {}", e);
                }
            }

            // Redirect stdin
            match OpenOptions::new().read(true).open("CONIN$") {
                Ok(conin) => {
                    let handle = conin.as_raw_handle();
                    if SetStdHandle(STD_INPUT_HANDLE, handle) == 0 {
                        eprintln!(
                            "[debug] Failed to set STD_INPUT_HANDLE: GetLastError() = {}",
                            GetLastError()
                        );
                    }
                    std::mem::forget(conin); // Leak handle so it stays open for the lifetime of the process
                }
                Err(e) => {
                    eprintln!("[debug] Failed to open CONIN$: {}", e);
                }
            }
        }
    }
}
