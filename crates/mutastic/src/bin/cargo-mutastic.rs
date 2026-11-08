//! `cargo mutastic …`: Cargo runs this as `cargo-mutastic mutastic …`, with the subcommand's name
//! as the first argument, which `cli::main` drops.
fn main() -> std::process::ExitCode {
    mutastic::cli::main(std::env::args_os().collect())
}
