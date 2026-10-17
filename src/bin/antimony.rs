//! Optional command-line adapter; language semantics remain in the library.
mod antimony_cli;

fn main() -> std::process::ExitCode {
    antimony_cli::main(std::env::args_os().skip(1).collect())
}
