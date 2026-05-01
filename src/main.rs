use std::process::ExitCode;

fn main() -> ExitCode {
    match books_for_bots::run_from_args() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("books-for-bots: {e}");
            ExitCode::from(1)
        }
    }
}
