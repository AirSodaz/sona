use std::process::ExitCode;

fn main() -> ExitCode {
    sona_runtime_fs::init_cli_shared_library_directory();

    match sona_cli::run_cli_from_args(std::env::args_os()) {
        Ok(output) => {
            if !output.stdout.is_empty() {
                println!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprintln!("{}", output.stderr);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
