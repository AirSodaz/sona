use clap::{Args, Subcommand};
use sona_core::project::{ProjectError, ProjectRecord, ProjectRepositorySnapshot};
use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::{Database, SqliteProjectAdapter};
use std::path::PathBuf;
use std::sync::Arc;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct ProjectsArgs {
    #[command(subcommand)]
    command: ProjectsCommands,
}

#[derive(Debug, Subcommand)]
enum ProjectsCommands {
    /// Lists persisted projects.
    List(ProjectsListArgs),
}

#[derive(Debug, Args)]
struct ProjectsListArgs {
    /// Application data directory containing the Sona database.
    #[arg(long, value_name = "PATH")]
    app_data_dir: PathBuf,
    /// Prints JSON instead of the default table output.
    #[arg(long)]
    json: bool,
}

pub fn run_projects(args: ProjectsArgs) -> CliResult<CliOutput> {
    match args.command {
        ProjectsCommands::List(args) => run_projects_list(args),
    }
}

fn run_projects_list(args: ProjectsListArgs) -> CliResult<CliOutput> {
    let database = Database::open_read_only(&args.app_data_dir)
        .map_err(|error| CliError::Io(error.to_string()))?;
    let state = SqliteProjectAdapter::new(
        Arc::new(database),
        Arc::new(UuidGenerator),
        Arc::new(SystemClock),
    )
    .load_state()
    .map_err(map_project_error)?;
    let output = if args.json {
        serde_json::to_string_pretty(&state)
            .map_err(|error| CliError::Serialize(error.to_string()))?
    } else {
        render_projects_table(&state)
    };

    Ok(CliOutput::stdout(output))
}

fn map_project_error(error: ProjectError) -> CliError {
    let message = error.to_string();
    match error {
        ProjectError::Serialization(_) => CliError::Serialize(message),
        ProjectError::Repository(_) | ProjectError::Clock(_) => CliError::Io(message),
    }
}

fn render_projects_table(state: &ProjectRepositorySnapshot) -> String {
    let rows = state
        .projects
        .iter()
        .map(|project| project_row(project, state.active_project_id.as_deref()))
        .collect::<Vec<_>>();
    let headers = ["ID", "NAME", "ACTIVE", "UPDATED"];
    let widths = column_widths(&headers, &rows);

    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in rows {
        let values = [
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
        ];
        append_table_row(&mut output, &values, &widths);
    }
    output
}

fn project_row(project: &ProjectRecord, active_project_id: Option<&str>) -> [String; 4] {
    [
        sanitize_table_cell(&project.id),
        sanitize_table_cell(&project.name),
        (active_project_id == Some(project.id.as_str())).to_string(),
        project.updated_at.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::map_project_error;
    use crate::CliError;
    use sona_core::ports::time::ClockError;
    use sona_core::project::ProjectError;

    fn json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{").unwrap_err()
    }

    #[test]
    fn maps_project_error_variants_to_cli_categories() {
        assert!(matches!(
            map_project_error(ProjectError::Serialization(json_error())),
            CliError::Serialize(_)
        ));
        assert!(matches!(
            map_project_error(ProjectError::Repository("database unavailable".to_string())),
            CliError::Io(_)
        ));
        assert!(matches!(
            map_project_error(ProjectError::Clock(ClockError::Unavailable(
                "system clock unavailable".to_string()
            ))),
            CliError::Io(_)
        ));
    }
}
