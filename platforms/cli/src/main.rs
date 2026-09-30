use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match sona_cli::execute_cli_from_args(std::env::args_os()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
