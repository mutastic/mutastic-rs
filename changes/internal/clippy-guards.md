- **Clippy holds processes, the environment, printing and `expect` to where they're used today.**
  `Command::new`, `env::var` and `env::var_os` are disallowed in `clippy.toml`, and each file that
  may use them says why in an `#![expect]` at its top, as Clippy can't allow a method by path. The
  library denies `println!` and `eprintln!`, which panic on a closed stream, and `expect_used` warns
  outside tests, each use outside them with its reason.
