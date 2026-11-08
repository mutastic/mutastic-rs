fn main() -> std::process::ExitCode {
    mutastic::cli::main(std::env::args_os().collect())
}
