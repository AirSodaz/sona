//! Verification harness proving that view structs have small, healthy sizes
//! and comfortably fit on standard 1MB Windows thread stacks without any artificial stack inflation.

use sona_desktop_lib::views::MainShell;
use sona_desktop_lib::views::settings::SettingsModal;
use sona_desktop_lib::views::transcript::TranscriptWorkbench;

#[test]
fn verify_view_struct_sizes_fit_comfortably_on_default_1mb_stack() {
    let shell_size = std::mem::size_of::<MainShell>();
    let settings_size = std::mem::size_of::<SettingsModal>();
    let workbench_size = std::mem::size_of::<TranscriptWorkbench>();

    println!("--- View Struct Memory Layout ---");
    println!(
        "MainShell struct size:            {shell_size} bytes ({:.2} KB)",
        shell_size as f64 / 1024.0
    );
    println!(
        "SettingsModal struct size:        {settings_size} bytes ({:.2} KB)",
        settings_size as f64 / 1024.0
    );
    println!(
        "TranscriptWorkbench struct size:  {workbench_size} bytes ({:.2} KB)",
        workbench_size as f64 / 1024.0
    );
    println!("Standard Windows thread stack:    1,048,576 bytes (1024.00 KB)");

    // On standard Windows default thread stacks (1MB = 1048576 bytes), stack frames must never approach
    // the 1MB limit. With autonomous entities and heap-allocated entity pointers, each struct is small.
    // Assert that each struct is comfortably below 50KB (less than 5% of default stack).
    assert!(
        shell_size < 50_000,
        "MainShell size ({shell_size} bytes) exceeded 50KB threshold"
    );
    assert!(
        settings_size < 50_000,
        "SettingsModal size ({settings_size} bytes) exceeded 50KB threshold"
    );
    assert!(
        workbench_size < 50_000,
        "TranscriptWorkbench size ({workbench_size} bytes) exceeded 50KB threshold"
    );
}
