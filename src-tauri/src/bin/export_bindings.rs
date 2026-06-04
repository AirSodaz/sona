fn main() {
    tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<tauri_appsona_lib::core::domain::LlmProvider>()
        .typ::<tauri_appsona_lib::core::domain::PolishPresetId>()
        .typ::<tauri_appsona_lib::core::domain::SummaryTemplateId>()
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .unwrap();
}
