use std::process::ExitCode;

fn greeting(name: &str) -> String {
    format!("hello {name}")
}

fn main() -> ExitCode {
    let name = std::env::args().nth(1).unwrap_or_else(|| "world".to_string());
    println!("{}", greeting(&name));
    if name.is_empty() {
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
