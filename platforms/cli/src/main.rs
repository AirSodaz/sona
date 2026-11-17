use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    sona_cli::logger::setup_cli_panic_hook();
    match sona_cli::execute_cli_from_args(std::env::args_os()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
