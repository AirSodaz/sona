use std::process::ExitCode;

fn main() -> ExitCode {
    match sona_cli::execute_cli_from_args(std::env::args_os()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
