use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::preset_models::{is_preset_model_installed_at, preset_models};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliModelSummary {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub language: String,
    pub size: String,
    pub modes: Vec<String>,
    pub installed: bool,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelListFilter {
    pub mode: Option<String>,
    pub model_type: Option<String>,
    pub language: Option<String>,
    pub installed_only: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelListEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub language: String,
    pub size: String,
    pub modes: Vec<String>,
    pub installed: bool,
    pub install_path: String,
}

pub fn list_cli_models(models_dir: &Path) -> Vec<CliModelSummary> {
    preset_models()
        .iter()
        .map(|model| {
            let install_path = model.resolve_install_path(models_dir);
            CliModelSummary {
                id: model.id.clone(),
                name: model.name.clone(),
                model_type: model.model_type.clone(),
                language: model.language.clone(),
                size: model.size.clone(),
                modes: model.modes.clone().unwrap_or_default(),
                installed: is_preset_model_installed_at(model, models_dir),
                install_path,
            }
        })
        .collect()
}

pub fn select_cli_models(
    models: Vec<CliModelSummary>,
    filter: &ModelListFilter,
) -> Vec<CliModelSummary> {
    let language_filter = filter.language.as_deref().map(str::to_lowercase);
    models
        .into_iter()
        .filter(|model| {
            filter
                .mode
                .as_deref()
                .map(|mode| model.modes.iter().any(|item| item == mode))
                .unwrap_or(true)
        })
        .filter(|model| {
            filter
                .model_type
                .as_deref()
                .map(|model_type| model.model_type == model_type)
                .unwrap_or(true)
        })
        .filter(|model| {
            language_filter
                .as_deref()
                .map(|language| {
                    model
                        .language
                        .split(',')
                        .any(|item| item.trim().eq_ignore_ascii_case(language))
                })
                .unwrap_or(true)
        })
        .filter(|model| !filter.installed_only || model.installed)
        .collect()
}

pub fn render_cli_model_table(models: &[CliModelSummary]) -> String {
    let rows = models
        .iter()
        .map(|model| {
            [
                model.id.clone(),
                model.model_type.clone(),
                model.language.clone(),
                model.size.clone(),
                if model.installed { "yes" } else { "no" }.to_string(),
                model.modes.join(","),
            ]
        })
        .collect::<Vec<_>>();
    let headers = ["ID", "Type", "Language", "Size", "Installed", "Modes"];
    let mut widths = headers.map(str::len);

    for row in &rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in rows {
        let refs = [
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
            row[4].as_str(),
            row[5].as_str(),
        ];
        append_table_row(&mut output, &refs, &widths);
    }
    output
}

pub fn remove_model_install_path(install_path: &Path) -> Result<(), String> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "Failed to inspect model path {}: {error}",
                install_path.display()
            ));
        }
    };

    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(install_path).map_err(|error| {
            format!(
                "Failed to delete model directory {}: {error}",
                install_path.display()
            )
        })
    } else {
        std::fs::remove_file(install_path).map_err(|error| {
            format!(
                "Failed to delete model file {}: {error}",
                install_path.display()
            )
        })
    }
}

impl From<CliModelSummary> for ModelListEntry {
    fn from(model: CliModelSummary) -> Self {
        Self {
            id: model.id,
            name: model.name,
            model_type: model.model_type,
            language: model.language,
            size: model.size,
            modes: model.modes,
            installed: model.installed,
            install_path: model.install_path.to_string_lossy().to_string(),
        }
    }
}

fn append_table_row(output: &mut String, values: &[&str; 6], widths: &[usize; 6]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&format!("{value:<width$}", width = widths[index]));
    }
    output.push('\n');
}

fn append_table_separator(output: &mut String, widths: &[usize; 6]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');
}
