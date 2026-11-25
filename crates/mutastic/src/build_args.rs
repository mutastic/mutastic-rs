//! The arguments Cargo gets on each call that builds the package: those after `--`, else
//! `MUTASTIC_BUILD_ARGS`, else mutastic.yaml's `build:` `args:`. The first given is used whole, in
//! place of the others. Only those typed after `--` are repeated in the commands mutastic offers: one
//! from the variable or the file applies by itself, and may be a secret.

use std::collections::BTreeSet;
use std::hash::{BuildHasher as _, Hasher as _};
use std::path::Path;
use std::sync::OnceLock;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::failure::{Code, Failure};

/// The variable, read when nothing was typed after `--`.
pub const VARIABLE: &str = "MUTASTIC_BUILD_ARGS";

/// Carries a gate's arguments to its count, a process of its own: never in a file.
pub const FOR_COUNT: &str = "MUTASTIC_GATE_COUNT_ARGS";

/// Between two arguments in [`FOR_COUNT`].
const SEPARATOR: char = '\u{1f}';

static TYPED: OnceLock<Vec<String>> = OnceLock::new();
static WORDS: OnceLock<Vec<String>> = OnceLock::new();

/// The line split at its first `--`: mutastic's own, and the build's, if there is one.
pub fn split_line(args: &[String]) -> (Vec<String>, Option<Vec<String>>) {
    match args.iter().position(|arg| arg == "--") {
        Some(at) => (args[..at].to_vec(), Some(args[at + 1..].to_vec())),
        None => (args.to_vec(), None),
    }
}

/// Keeps what was typed after `--`, for the whole process: one command runs in it.
pub fn typed(arguments: Vec<String>) {
    let _ = TYPED.set(arguments);
}

/// Settles what Cargo gets, refusing what fights how mutastic drives it. `configured` is the
/// file's, each with where it's written.
pub fn settle(configured: Option<&[(String, String)]>) -> Result<(), Failure> {
    let words = match (TYPED.get(), variable()) {
        (Some(typed), _) => {
            check(typed).map_err(|(_, why)| Code::OptionValue.with(why))?;
            typed.clone()
        }
        (None, Some(set)) => {
            let words =
                split(&set).map_err(|why| Code::Environment.with(format!("{VARIABLE}: {why}")))?;
            check(&words)
                .map_err(|(_, why)| Code::Environment.with(format!("{VARIABLE}: {why}")))?;
            words
        }
        (None, None) => {
            let listed = configured.unwrap_or_default();
            let words: Vec<String> = listed.iter().map(|(word, _)| word.clone()).collect();
            check(&words).map_err(|(at, why)| {
                let from = listed.get(at).map_or("mutastic.yaml", |(_, from)| from);
                Code::Config.with(format!("{from}: {why}"))
            })?;
            words
        }
    };
    let _ = WORDS.set(words);
    Ok(())
}

/// The variable, when it's set to something.
fn variable() -> Option<String> {
    std::env::var(VARIABLE)
        .ok()
        .filter(|set| !set.trim().is_empty())
}

/// What the gate's count was given by the `gate open` that started it.
pub fn from_gate() {
    let given = std::env::var(FOR_COUNT).unwrap_or_default();
    let words = if given.is_empty() {
        Vec::new()
    } else {
        given.split(SEPARATOR).map(str::to_string).collect()
    };
    let _ = WORDS.set(words);
}

/// [`words`], as [`FOR_COUNT`] carries them.
pub fn for_count() -> String {
    words().join(&SEPARATOR.to_string())
}

/// What Cargo gets.
pub fn words() -> &'static [String] {
    WORDS.get().map_or(&[], Vec::as_slice)
}

/// ` -- --features postgres`, the end of each command mutastic offers: empty when nothing was typed.
pub fn ending() -> String {
    TYPED
        .get()
        .map_or_else(String::new, |typed| format!(" --{}", quoted(typed)))
}

/// ` --features postgres`, for a `cargo` command offered.
pub fn cargo_ending() -> String {
    TYPED.get().map_or_else(String::new, |typed| quoted(typed))
}

fn quoted(words: &[String]) -> String {
    words
        .iter()
        .map(|word| format!(" {}", shell(word)))
        .collect()
}

/// `word` as a shell reads it back.
fn shell(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_./:=,@%+-".contains(c);
    if !word.is_empty() && word.chars().all(plain) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// Each of mutastic's commands in `message`, on a line of its own, ending in what was typed.
pub fn with_ending(message: &str) -> String {
    let ending = ending();
    if ending.is_empty() {
        return message.to_string();
    }
    ended(message, &ending)
}

/// Once: some are offered with it already.
fn ended(message: &str, ending: &str) -> String {
    let commands = [
        "run", "verify", "explain", "ignore", "next", "baseline", "gate", "mutants",
    ];
    message
        .split('\n')
        .map(|line| {
            let offered = line
                .strip_prefix("  mutastic ")
                .and_then(|rest| rest.split(' ').next())
                .is_some_and(|command| commands.contains(&command));
            if offered && !line.ends_with(ending) && !line.ends_with(" --help") {
                format!("{line}{ending}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A variable's words: split at spaces, but not inside single or double quotes, which go. A
/// backslash is kept, and nothing is expanded.
pub fn split(text: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote = None;
    for next in text.chars() {
        match quote {
            Some(open) if next == open => quote = None,
            Some(_) => word.get_or_insert_with(String::new).push(next),
            None if next == '\'' || next == '"' => {
                quote = Some(next);
                word.get_or_insert_with(String::new);
            }
            None if next.is_whitespace() => words.extend(word.take()),
            None => word.get_or_insert_with(String::new).push(next),
        }
    }
    if let Some(open) = quote {
        return Err(format!("a {open} isn't closed"));
    }
    words.extend(word);
    Ok(words)
}

const TARGETS: &str = "mutastic chooses the tests and targets it builds";
const WHERE: &str = "mutastic chooses the package and where it's built, by what it's run on";
const PROFILE: &str = "mutastic builds with a profile of its own";
const OUTPUT: &str = "mutastic reads the build's output, which it changes";
const NO_BUILD: &str = "the build wouldn't compile what mutastic runs";
const TARGET: &str = "mutastic runs the tests itself: CARGO_BUILD_TARGET chooses the target";
const GIVEN: &str = "mutastic gives it already";
const COMMAND: &str = "mutastic chooses the cargo command it runs";

/// Cargo's options that fight how mutastic drives it, by name without dashes.
const REFUSED: &[(&str, &str)] = &[
    ("p", WHERE),
    ("package", WHERE),
    ("workspace", WHERE),
    ("all", WHERE),
    ("exclude", WHERE),
    ("manifest-path", WHERE),
    ("target-dir", WHERE),
    ("artifact-dir", WHERE),
    ("out-dir", WHERE),
    ("C", WHERE),
    ("lib", TARGETS),
    ("bin", TARGETS),
    ("bins", TARGETS),
    ("example", TARGETS),
    ("examples", TARGETS),
    ("test", TARGETS),
    ("tests", TARGETS),
    ("bench", TARGETS),
    ("benches", TARGETS),
    ("all-targets", TARGETS),
    ("doc", TARGETS),
    ("r", PROFILE),
    ("release", PROFILE),
    ("profile", PROFILE),
    ("message-format", OUTPUT),
    ("color", OUTPUT),
    ("q", OUTPUT),
    ("quiet", OUTPUT),
    ("no-run", NO_BUILD),
    ("build-plan", NO_BUILD),
    ("unit-graph", NO_BUILD),
    ("list", NO_BUILD),
    ("h", NO_BUILD),
    ("help", NO_BUILD),
    ("V", NO_BUILD),
    ("version", NO_BUILD),
    ("explain", NO_BUILD),
    ("target", TARGET),
    ("keep-going", GIVEN),
];

/// Settings of `--config` that would undo mutastic's own.
const SETTINGS: &[(&str, &str)] = &[
    ("profile", PROFILE),
    ("build.target-dir", WHERE),
    ("build.build-dir", WHERE),
    ("build.target", TARGET),
    ("build.rustc-wrapper", NO_BUILD),
    ("build.rustc-workspace-wrapper", NO_BUILD),
    ("build.rustc", NO_BUILD),
    ("term", OUTPUT),
];

/// Options whose value can be the word after them.
const WITH_VALUE: &[&str] = &["F", "features", "j", "jobs", "config", "Z", "lockfile-path"];

/// The first refused, where it is, and why, named without its value, which may be a secret.
fn check(words: &[String]) -> Result<(), (usize, String)> {
    let mut at = 0;
    while at < words.len() {
        let here = at;
        at += 1;
        let word = &words[here];
        let refuse =
            |name: &str, why: &str| Err((here, format!("{name} can't be passed to Cargo: {why}")));
        let Some((name, value)) = option(word) else {
            // It may be the value of the option before it, so it's never said.
            return refuse("a word that isn't an option", COMMAND);
        };
        let dashes = if word.starts_with("--") { "--" } else { "-" };
        if let Some((_, why)) = REFUSED.iter().find(|(refused, _)| *refused == name) {
            return refuse(&format!("{dashes}{name}"), why);
        }
        let takes = WITH_VALUE.contains(&name.as_str());
        let value = match value {
            None if takes => {
                at += 1;
                words.get(here + 1).cloned()
            }
            given => given,
        };
        if name == "config" {
            let setting = value.unwrap_or_default();
            let key = setting
                .split('=')
                .next()
                .unwrap_or_default()
                .trim()
                .replace(['"', '\''], "");
            let found = SETTINGS
                .iter()
                .find(|(own, _)| key == *own || key.starts_with(&format!("{own}.")));
            if let Some((own, why)) = found {
                return refuse(&format!("--config {own}"), why);
            }
        } else if dashes == "-" && !takes {
            // `-qv`: options of one letter given together.
            for letter in word[1..].chars().map(String::from) {
                if let Some((_, why)) = REFUSED.iter().find(|(refused, _)| *refused == letter) {
                    return refuse(&format!("-{letter}"), why);
                }
            }
        }
    }
    Ok(())
}

/// An option's name and the value with it: `--features=a`, `-Fa`. `None` for any other word.
fn option(word: &str) -> Option<(String, Option<String>)> {
    if let Some(long) = word.strip_prefix("--").filter(|long| !long.is_empty()) {
        return Some(match long.split_once('=') {
            Some((name, value)) => (name.to_string(), Some(value.to_string())),
            None => (long.to_string(), None),
        });
    }
    let short = word
        .strip_prefix('-')
        .filter(|short| !short.is_empty() && !short.starts_with('-'))?;
    let letter = short.chars().next()?.to_string();
    if ["F", "j", "Z", "p", "C"].contains(&letter.as_str()) && short.len() > 1 {
        let value = short[1..].trim_start_matches('=').to_string();
        return Some((letter, Some(value)));
    }
    Some((short.to_string(), None))
}

/// The features the build turns on, by what Cargo gets: the default ones, unless told not to, and
/// those named, and what they enable, from the package's `features` in `cargo metadata`. Of
/// `package`'s `name/feature`, only its own.
pub fn features(table: &Value, package: &str) -> BTreeSet<String> {
    features_of(words(), table, package)
}

fn features_of(words: &[String], table: &Value, package: &str) -> BTreeSet<String> {
    let mut named: Vec<String> = Vec::new();
    let mut default = true;
    let mut at = 0;
    while at < words.len() {
        let word = &words[at];
        at += 1;
        let Some((name, value)) = option(word) else {
            continue;
        };
        let value = if value.is_none() && WITH_VALUE.contains(&name.as_str()) {
            at += 1;
            words.get(at - 1).cloned()
        } else {
            value
        };
        match name.as_str() {
            "no-default-features" => default = false,
            "all-features" => {
                named.extend(
                    table
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(k, _)| k.clone()),
                );
            }
            "features" | "F" => named.extend(
                value
                    .unwrap_or_default()
                    .split([',', ' '])
                    .map(|feature| match feature.split_once('/') {
                        Some((of, own)) if of == package => own.to_string(),
                        // Another package's feature: of a dependency, which it turns on.
                        Some((of, _)) => of.trim_end_matches('?').to_string(),
                        None => feature.to_string(),
                    })
                    .filter(|feature| !feature.is_empty()),
            ),
            _ => {}
        }
    }
    if default {
        named.push("default".to_string());
    }
    crate::cfg::enabled(table, named)
}

/// What a share says of the features its build turned on, so one is never put together with shares
/// built with others: the code's hash is of the sources, which are the same whatever is on. Empty
/// when the arguments choose none.
pub fn share_features(project: &crate::workspace::Project) -> String {
    features_chosen(words(), &project.features)
}

fn features_chosen(words: &[String], on: &BTreeSet<String>) -> String {
    let mut named = BTreeSet::new();
    let mut chosen = false;
    let mut at = 0;
    while at < words.len() {
        let word = &words[at];
        at += 1;
        let Some((name, value)) = option(word) else {
            continue;
        };
        let value = if value.is_none() && WITH_VALUE.contains(&name.as_str()) {
            at += 1;
            words.get(at - 1).cloned()
        } else {
            value
        };
        match name.as_str() {
            "no-default-features" | "all-features" => chosen = true,
            "features" | "F" => {
                chosen = true;
                // A dependency's: the package's own are in what's on.
                let others = value.unwrap_or_default();
                let others = others
                    .split([',', ' '])
                    .filter(|feature| feature.contains('/'));
                named.extend(others.map(str::to_string));
            }
            _ => {}
        }
    }
    if !chosen {
        return String::new();
    }
    let on = on
        .iter()
        .map(String::as_str)
        .filter(|feature| *feature != "default");
    let all: BTreeSet<&str> = on.chain(named.iter().map(String::as_str)).collect();
    format!("features {}", all.into_iter().collect::<Vec<_>>().join(","))
}

/// What stands for the arguments in a record, so a run with others is never the same code: empty for
/// none, so a record of a run without any is as it always was. An argument may be a short secret, and
/// a plain hash of one can be guessed: it's an HMAC, under a random key that stays in `work`.
pub fn key(work: &Path) -> Result<String, Failure> {
    key_of(words(), work)
        .map_err(|error| crate::failure::disk(format!("{}: {error}", work.display())))
}

fn key_of(words: &[String], work: &Path) -> std::io::Result<String> {
    if words.is_empty() {
        return Ok(String::new());
    }
    let key = secret(work)?;
    let mut message = Vec::new();
    for word in words {
        message.extend_from_slice(word.as_bytes());
        message.push(0);
    }
    let padded = |pad: u8| -> Vec<u8> {
        let mut block = [pad; 64];
        for (byte, of_key) in block.iter_mut().zip(&key) {
            *byte ^= of_key;
        }
        block.to_vec()
    };
    let inner = Sha256::new()
        .chain_update(padded(0x36))
        .chain_update(&message)
        .finalize();
    let outer = Sha256::new()
        .chain_update(padded(0x5c))
        .chain_update(inner)
        .finalize();
    Ok(outer.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The work directory's key, made the first time. Only its owner reads it.
fn secret(work: &Path) -> std::io::Result<Vec<u8>> {
    let file = work.join("build-args.key");
    if let Ok(kept) = std::fs::read(&file) {
        if kept.len() == 32 {
            return Ok(kept);
        }
    }
    std::fs::create_dir_all(work)?;
    let key = random();
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::io::Write::write_all(&mut options.open(&file)?, &key)?;
    Ok(key)
}

/// 32 bytes from the system's randomness: where it has a file of it, that, and everywhere the keys
/// std seeds its hash maps with from it.
fn random() -> Vec<u8> {
    let mut hasher = Sha256::new();
    for turn in 0..4u64 {
        let mut seeded = std::collections::hash_map::RandomState::new().build_hasher();
        seeded.write_u64(turn);
        hasher.update(seeded.finish().to_le_bytes());
    }
    #[cfg(unix)]
    if let Ok(mut source) = std::fs::File::open("/dev/urandom") {
        let mut bytes = [0u8; 32];
        if std::io::Read::read_exact(&mut source, &mut bytes).is_ok() {
            hasher.update(bytes);
        }
    }
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_string).collect()
    }

    #[test]
    fn a_variable_splits_at_spaces_but_not_inside_quotes() {
        assert_eq!(
            split("  --offline\t--config='a b' \"it's\" '' C:\\x ").unwrap(),
            ["--offline", "--config=a b", "it's", "", "C:\\x"]
        );
        assert_eq!(split(" ").unwrap(), Vec::<String>::new());
        assert_eq!(split("-F 'a"), Err("a ' isn't closed".to_string()));
    }

    #[test]
    fn the_line_splits_at_its_first_dashes() {
        let (own, build) = split_line(&words("run x -- --edition=java -- y"));
        assert_eq!(own, ["run", "x"]);
        assert_eq!(build.unwrap(), ["--edition=java", "--", "y"]);
        assert_eq!(split_line(&words("run x")).1, None);
        assert_eq!(split_line(&words("run --")).1, Some(vec![]));
    }

    #[test]
    fn an_argument_for_the_build_is_taken() {
        for line in [
            "--features postgres",
            "--features=postgres,tls",
            "-F postgres",
            "-Fpostgres",
            "--no-default-features",
            "--all-features",
            "--offline",
            "--locked",
            "-j4",
            "--jobs 4",
            "-v",
            "--config net.git-fetch-with-cli=true",
            "-Z unstable-options",
        ] {
            assert_eq!(check(&words(line)), Ok(()), "{line}");
        }
    }

    #[test]
    fn one_that_fights_mutastic_is_refused_without_its_value() {
        for line in [
            "--test secret",
            "--test=secret",
            "-p secret",
            "-psecret",
            "--release",
            "-r",
            "-vq",
            "--profile=secret",
            "--message-format=secret",
            "--target-dir=secret",
            "--manifest-path secret",
            "--no-run",
            "--target=secret",
            "--config profile.dev.opt-level=\"secret\"",
            "--config=build.target-dir='secret'",
            "secret",
            "--unknown secret",
            "--offline secret",
            "-Zsecret secret",
            "--config profile.secret.opt-level=1",
            "+nightly",
            "--",
            "--help",
        ] {
            let (_, why) = check(&words(line)).expect_err(line);
            assert!(!why.contains("secret"), "{why}");
        }
        assert_eq!(
            check(&words("--offline --test=x")),
            Err((1, format!("--test can't be passed to Cargo: {TARGETS}")))
        );
    }

    #[test]
    fn the_features_are_those_the_build_turns_on() {
        let table = serde_json::json!({
            "default": ["std"], "std": [], "postgres": ["sql"], "sql": [], "tls": ["dep:rustls"]
        });
        let on = |line: &str| {
            let words = if line.is_empty() { vec![] } else { words(line) };
            features_of(&words, &table, "shop")
                .into_iter()
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(on(""), "default std");
        assert_eq!(on("--features postgres"), "default postgres sql std");
        assert_eq!(on("-Fshop/tls --no-default-features"), "tls");
        assert_eq!(on("--features=tls,sql --no-default-features"), "sql tls");
        assert_eq!(on("--all-features"), "default postgres sql std tls");
    }

    #[test]
    fn the_arguments_stand_in_a_record_under_a_key_of_the_work_directorys() {
        let work = std::env::temp_dir().join(format!("mutastic-args-key-{}", std::process::id()));
        let of = |line: &str| key_of(&words(line), &work).unwrap();
        assert_eq!(of("--config=pin=1234"), of("--config=pin=1234"));
        assert_ne!(of("--config=pin=1234"), of("--config=pin=1235"));
        assert_ne!(of("a b"), of("b a"));
        assert_ne!(of("a b"), of("ab"));
        assert_eq!(key_of(&[], &work).unwrap(), "");
        let kept = of("a");
        std::fs::remove_dir_all(&work).unwrap();
        assert_ne!(of("a"), kept);
        std::fs::remove_dir_all(&work).unwrap();
    }

    #[test]
    fn a_command_offered_ends_in_what_was_typed_once() {
        let end = " -- -Fa";
        for (line, expected) in [
            ("  mutastic run 'my dir'", "  mutastic run 'my dir' -- -Fa"),
            ("  mutastic run 'a -- b'", "  mutastic run 'a -- b' -- -Fa"),
            ("  mutastic run x -- -Fa", "  mutastic run x -- -Fa"),
            ("  mutastic run --help", "  mutastic run --help"),
            ("  cargo test", "  cargo test"),
            (
                "a\n  mutastic verify m1\n",
                "a\n  mutastic verify m1 -- -Fa\n",
            ),
        ] {
            assert_eq!(ended(line, end), expected);
        }
        assert_eq!(ended("  mutastic run x", " --"), "  mutastic run x --");
        assert_eq!(ended("  mutastic run x --", " --"), "  mutastic run x --");
    }

    #[test]
    fn a_share_says_the_features_chosen_however_they_are_written() {
        let on = |names: &[&str]| names.iter().map(|name| name.to_string()).collect();
        let said = |line: &str, names: &[&str]| features_chosen(&words(line), &on(names));
        assert_eq!(said("--offline", &["default"]), "");
        assert_eq!(said("-Fgift", &["default", "gift"]), "features gift");
        assert_eq!(said("--features gift", &["gift"]), "features gift");
        assert_eq!(
            said("--features=serde/std", &["serde"]),
            "features serde,serde/std"
        );
        assert_eq!(said("--no-default-features", &[]), "features ");
    }

    #[test]
    fn a_word_is_quoted_for_a_shell() {
        assert_eq!(
            quoted(&[
                "-Fa".into(),
                "a b".into(),
                "it's".into(),
                String::new(),
                "$X".into()
            ]),
            " -Fa 'a b' 'it'\\''s' '' '$X'"
        );
    }
}
