
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_bindings() {
        tauri_specta::Builder::<tauri::Wry>::new()
            .typ::<LlmProvider>()
            .typ::<PolishPresetId>()
            .typ::<SummaryTemplateId>()
            .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
            .unwrap();
    }
}
