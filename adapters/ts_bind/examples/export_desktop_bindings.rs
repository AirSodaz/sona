use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let frontend_root = repository_root.join("platforms/desktop/frontend");
    let bindings_path = sona_ts_bind::desktop_bindings_output(frontend_root);
    let generated = sona_ts_bind::render_desktop_typescript_bindings()?;
    let changed = fs::read_to_string(&bindings_path).ok().as_deref() != Some(generated.as_str());

    if changed {
        fs::write(&bindings_path, generated)?;
    }
    println!(
        "{} {}",
        if changed { "updated" } else { "unchanged" },
        bindings_path.display()
    );
    Ok(())
}
