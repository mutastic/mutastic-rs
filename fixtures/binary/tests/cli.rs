//! The binary, run as a separate process the way a person runs it: a mutant of `main` reaches it.

use std::process::Command;

fn run(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_greet"))
        .args(args)
        .output()
        .unwrap();
    (out.status.code().unwrap(), String::from_utf8(out.stdout).unwrap())
}

#[test]
fn greets_the_world() {
    assert_eq!(run(&[]), (0, "hello world\n".to_string()));
}

#[test]
fn greets_a_name() {
    assert_eq!(run(&["ada"]).1, "hello ada\n");
}

#[test]
fn an_empty_name_exits_with_two() {
    assert_eq!(run(&[""]).0, 2);
}
