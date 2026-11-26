//! Which `cfg` predicates hold in the build mutastic runs, so code this build doesn't have isn't
//! mutated: its mutants could never be reached, and would count against the score as not covered.
//!
//! The best evidence is the build's own. Each time the copy's crate is compiled, the wrapper asks
//! the same rustc `--print cfg` with the flags that decide it (the target, the codegen options, and
//! every `--cfg`: the features Cargo turned on, what the build script printed, and what `RUSTFLAGS`
//! and Cargo's config added), and keeps the answer beside the build ([`record`]). With that, a
//! predicate is judged as rustc judges it: a name nothing set is off, so `docsrs`, `loom` and a
//! build script's `cfg` it didn't print are all off, and `not(..)` of them on.
//!
//! Before a build has said, the target's settings come from rustc itself (`rustc --print cfg`, for
//! the target and flags Cargo would use), the features from the package's own `[features]` table
//! (the default ones, followed through what each enables), and `debug_assertions` is on because
//! mutastic's profile turns it on. A predicate is judged three ways: built, not built, or not
//! known. Only a predicate that is surely off leaves code out: a name rustc doesn't define for every
//! target is not known, and its code is mutated until the build says. Wrongly leaving code out
//! would hide a gap, where wrongly keeping it only shows it as not covered.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use ra_ap_syntax::ast::{self, HasAttrs};
use ra_ap_syntax::{AstNode, SyntaxNode};
use serde_json::Value;

/// Names whose every value rustc prints for a target: one it doesn't print is off.
const FROM_TARGET: &[&str] = &[
    "unix",
    "windows",
    "target_os",
    "target_family",
    "target_arch",
    "target_pointer_width",
    "target_endian",
    "target_env",
    "target_vendor",
    "target_abi",
    "target_has_atomic",
    "panic",
];

/// Names that are off in every build mutastic runs: documentation, doc tests, tools, and the
/// proc-macro crates mutastic doesn't mutate.
const NEVER: &[&str] = &["doc", "doctest", "miri", "rustfmt", "clippy", "proc_macro"];

/// Whether a predicate holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Truth {
    Yes,
    No,
    Unknown,
}

/// Set in a build's environment: the file the wrapper keeps the package's `cfg`s in.
pub const RECORD: &str = "MUTASTIC_CFG_RECORD";

/// Set in a build's environment: the directory of the package whose `cfg`s are kept.
pub const PACKAGE: &str = "MUTASTIC_CFG_PACKAGE";

/// What a build has on: the target's `cfg` values and the enabled features. Either can be not
/// known, and then every predicate on it is [`Truth::Unknown`]. When the set is the build's own
/// (`whole`), a name it doesn't have is off, as rustc has it.
#[derive(Clone, Debug, Default)]
pub struct Cfg {
    target: Option<BTreeSet<(String, Option<String>)>>,
    features: Option<BTreeSet<String>>,
    whole: bool,
}

impl Cfg {
    /// Nothing known: every piece of code counts as built.
    pub fn unknown() -> Cfg {
        Cfg::default()
    }

    /// From `rustc --print cfg`'s lines and the enabled features.
    pub fn new(printed: &str, features: BTreeSet<String>) -> Cfg {
        let target = parse(printed);
        Cfg {
            target: (!target.is_empty()).then_some(target),
            features: Some(features),
            whole: false,
        }
    }

    /// From what rustc printed for the build itself, every `--cfg` it was given included: the
    /// whole set, so a name it doesn't have is off. Nothing printed is nothing known.
    pub fn built(printed: &str) -> Cfg {
        let set = parse(printed);
        if set.is_empty() {
            return Cfg::unknown();
        }
        let features = set
            .iter()
            .filter(|(name, _)| name == "feature")
            .filter_map(|(_, value)| value.clone())
            .collect();
        Cfg {
            target: Some(set),
            features: Some(features),
            whole: true,
        }
    }

    /// The package's: what its last build in mutastic's copy had, when it has been built there,
    /// and otherwise this machine's, with `features` on.
    pub fn of(record: &Path, features: BTreeSet<String>) -> Cfg {
        match recorded(record) {
            Some(printed) => Cfg::built(&printed),
            None => Cfg::here(features),
        }
    }

    /// This machine's: asks rustc, as Cargo would run it, and takes `features` as on. When rustc
    /// can't be asked, the target is not known.
    pub fn here(features: BTreeSet<String>) -> Cfg {
        Cfg::new(&print_cfg().unwrap_or_default(), features)
    }

    /// Whether `predicate`, the inside of a `cfg(..)`, holds.
    pub fn holds(&self, predicate: &str) -> Truth {
        let predicate = predicate.trim();
        for (combinator, empty) in [("all(", Truth::Yes), ("any(", Truth::No)] {
            if let Some(args) = predicate
                .strip_prefix(combinator)
                .and_then(|rest| rest.strip_suffix(')'))
            {
                let parts = split(args);
                return parts.into_iter().map(|part| self.holds(part)).fold(
                    empty,
                    |sofar, next| match (empty, sofar, next) {
                        (Truth::Yes, Truth::No, _) | (Truth::Yes, _, Truth::No) => Truth::No,
                        (Truth::No, Truth::Yes, _) | (Truth::No, _, Truth::Yes) => Truth::Yes,
                        (_, Truth::Unknown, _) | (_, _, Truth::Unknown) => Truth::Unknown,
                        _ => empty,
                    },
                );
            }
        }
        if let Some(inner) = predicate
            .strip_prefix("not(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            return match self.holds(inner) {
                Truth::Yes => Truth::No,
                Truth::No => Truth::Yes,
                Truth::Unknown => Truth::Unknown,
            };
        }
        let (name, value) = match predicate.split_once('=') {
            Some((name, value)) => (name.trim(), Some(value.trim().trim_matches('"'))),
            None => (predicate, None),
        };
        self.name_holds(name, value)
    }

    fn name_holds(&self, name: &str, value: Option<&str>) -> Truth {
        let known = |yes: bool| if yes { Truth::Yes } else { Truth::No };
        match name {
            "feature" => match (&self.features, value) {
                (Some(features), Some(value)) => known(features.contains(value)),
                _ => Truth::Unknown,
            },
            "debug_assertions" | "true" => Truth::Yes,
            "false" => Truth::No,
            // The library is built both with its tests and without, for the integration tests.
            "test" => Truth::Unknown,
            _ if NEVER.contains(&name) => Truth::No,
            _ if self.whole || FROM_TARGET.contains(&name) => match &self.target {
                Some(target) => {
                    known(target.contains(&(name.to_string(), value.map(str::to_string))))
                }
                None => Truth::Unknown,
            },
            _ => Truth::Unknown,
        }
    }

    /// The innermost `cfg(..)` on `node` or around it that is surely off, as written
    /// (`cfg(windows)`), or `None` when this build has the code, as far as can be told.
    pub fn off_at(&self, node: &SyntaxNode) -> Option<String> {
        node.ancestors().find_map(|ancestor| {
            ast::AnyHasAttrs::cast(ancestor)?
                .attrs()
                .find_map(|attr| self.off_attribute(&attr))
        })
    }

    fn off_attribute(&self, attr: &ast::Attr) -> Option<String> {
        let text = crate::sites::plain(attr.syntax());
        let inner = text
            .strip_prefix("#![")
            .or_else(|| text.strip_prefix("#["))?
            .strip_suffix(']')?
            .trim();
        let predicate = inner
            .strip_prefix("cfg")?
            .trim_start()
            .strip_prefix('(')?
            .strip_suffix(')')?;
        (self.holds(predicate) == Truth::No).then(|| inner.to_string())
    }
}

/// `rustc --print cfg`'s lines, as names and values.
fn parse(printed: &str) -> BTreeSet<(String, Option<String>)> {
    printed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once('=') {
            Some((name, value)) => (
                name.trim().to_string(),
                Some(value.trim().trim_matches('"').to_string()),
            ),
            None => (line.to_string(), None),
        })
        .collect()
}

/// What the package's last build in mutastic's copy had on, as rustc printed it, when the wrapper
/// kept it.
pub fn recorded(record: &Path) -> Option<String> {
    std::fs::read_to_string(record)
        .ok()
        .filter(|printed| !printed.trim().is_empty())
}

/// In the wrapper, before rustc compiles: when `args` (the rustc and its arguments) compile the
/// package under test, not its build script, keeps what that rustc prints for
/// `--print cfg` with the flags that decide it, in the file [`RECORD`] names. Anything that goes
/// wrong keeps nothing: the run then judges from the text, as before a build.
pub fn record(args: &[OsString]) {
    let (Some(record), Some(package)) = (std::env::var_os(RECORD), std::env::var_os(PACKAGE))
    else {
        return;
    };
    let Some((rustc, args)) = args.split_first() else {
        return;
    };
    if !compiles_package(args, Path::new(&package)) {
        return;
    }
    let Ok(output) = std::process::Command::new(rustc)
        .args(["--print", "cfg"])
        .args(deciding(args))
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }
    let record = Path::new(&record);
    let Some(dir) = record.parent() else {
        return;
    };
    // Written whole and moved into place: the library and a binary may be compiled at once.
    let partial = dir.join(format!(".cfg-{}", std::process::id()));
    if std::fs::create_dir_all(dir).is_ok() && std::fs::write(&partial, &output.stdout).is_ok() {
        std::fs::rename(&partial, record).ok();
    }
}

/// Whether rustc's `args` compile a crate of the package in `package`, as Cargo runs it there, and
/// not its build script or a question Cargo asks. Its tests count: `cargo build --tests` compiles a
/// library only as its tests, with the library's flags and `--test`, which `--print cfg` isn't
/// given, as `cfg(test)` is judged on its own.
fn compiles_package(args: &[OsString], package: &Path) -> bool {
    let crate_name = args
        .iter()
        .position(|arg| arg == "--crate-name")
        .and_then(|at| args.get(at + 1));
    let ours = std::env::var_os("CARGO_MANIFEST_DIR")
        .is_some_and(|dir| same_dir(Path::new(&dir), package));
    ours && crate_name.is_some_and(|name| {
        let name = name.to_string_lossy();
        !name.starts_with("build_script_") && name != "___"
    }) && !args
        .iter()
        .any(|arg| arg.to_string_lossy().starts_with("--print"))
}

fn same_dir(one: &Path, other: &Path) -> bool {
    one == other
        || matches!(
            (std::fs::canonicalize(one), std::fs::canonicalize(other)),
            (Ok(one), Ok(other)) if one == other
        )
}

/// The arguments of a compile that decide what `--print cfg` says: the target and sysroot, every
/// `--cfg`, and the codegen options rustc reads `cfg`s from.
fn deciding(args: &[OsString]) -> Vec<OsString> {
    const CODEGEN: &[&str] = &[
        "target-feature",
        "target-cpu",
        "panic",
        "debug-assertions",
        "overflow-checks",
        "relocation-model",
    ];
    let decides = |codegen: &str| {
        CODEGEN.iter().any(|option| {
            codegen
                .strip_prefix(option)
                .is_some_and(|rest| rest.starts_with('='))
        })
    };
    let mut kept = Vec::new();
    let mut at = 0;
    while at < args.len() {
        let arg = args[at].to_string_lossy();
        let next = args.get(at + 1);
        match arg.as_ref() {
            "--target" | "--cfg" | "--sysroot" => {
                kept.extend([args[at].clone()].into_iter().chain(next.cloned()));
                at += 2;
                continue;
            }
            "-C" | "--codegen" => {
                if next.is_some_and(|option| decides(&option.to_string_lossy())) {
                    kept.extend([args[at].clone()].into_iter().chain(next.cloned()));
                }
                at += 2;
                continue;
            }
            _ => {}
        }
        let joined = ["--target=", "--cfg=", "--sysroot="]
            .iter()
            .any(|flag| arg.starts_with(flag));
        let codegen = arg
            .strip_prefix("-C")
            .or_else(|| arg.strip_prefix("--codegen="))
            .is_some_and(decides);
        if joined || codegen {
            kept.push(args[at].clone());
        }
        at += 1;
    }
    kept
}

/// `a, f(b, c), d` split at its top-level commas, without an empty last part.
fn split(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start, mut quoted) = (0i32, 0, false);
    for (at, c) in text.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if depth == 0 && !quoted => {
                parts.push(&text[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts.retain(|part| !part.trim().is_empty());
    parts
}

/// `rustc --print cfg` for the target Cargo builds for, with the flags it passes.
fn print_cfg() -> Option<String> {
    let mut command =
        std::process::Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()));
    command.args(["--print", "cfg"]);
    if let Ok(target) = std::env::var("CARGO_BUILD_TARGET") {
        command.args(["--target", &target]);
    }
    if let Ok(encoded) = std::env::var("CARGO_ENCODED_RUSTFLAGS") {
        command.args(encoded.split('\x1f').filter(|flag| !flag.is_empty()));
    } else if let Ok(flags) = std::env::var("RUSTFLAGS") {
        command.args(flags.split_whitespace());
    }
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The features a plain build of a package turns on: `default`, and everything it enables, from
/// the package's `features` in `cargo metadata`.
pub fn default_features(table: &Value) -> BTreeSet<String> {
    enabled(table, vec!["default".to_string()])
}

/// The features `named` turns on: themselves, those the package has, and everything they enable.
pub fn enabled(table: &Value, named: Vec<String>) -> BTreeSet<String> {
    let mut on = BTreeSet::new();
    let mut queue = named;
    while let Some(feature) = queue.pop() {
        let Some(enables) = table.get(&feature).and_then(Value::as_array) else {
            continue;
        };
        if !on.insert(feature) {
            continue;
        }
        for enabled in enables.iter().filter_map(Value::as_str) {
            // `dep:x` turns on a dependency, not a feature; `x/f` turns on `x` too, `x?/f` doesn't.
            if enabled.starts_with("dep:") || enabled.contains("?/") {
                continue;
            }
            let name = enabled.split('/').next().unwrap_or(enabled);
            queue.push(name.to_string());
        }
    }
    on
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: &str = "debug_assertions\npanic=\"unwind\"\ntarget_arch=\"aarch64\"\n\
        target_family=\"unix\"\ntarget_os=\"macos\"\ntarget_pointer_width=\"64\"\nunix\n";

    fn mac(features: &[&str]) -> Cfg {
        Cfg::new(MAC, features.iter().map(|f| f.to_string()).collect())
    }

    #[test]
    fn a_predicate_is_built_not_built_or_not_known() {
        let cfg = mac(&["std"]);
        for (predicate, truth) in [
            ("unix", Truth::Yes),
            ("windows", Truth::No),
            ("target_os = \"macos\"", Truth::Yes),
            ("target_os=\"linux\"", Truth::No),
            ("feature = \"std\"", Truth::Yes),
            ("feature = \"simd\"", Truth::No),
            ("debug_assertions", Truth::Yes),
            ("doc", Truth::No),
            ("docsrs", Truth::Unknown),
            ("test", Truth::Unknown),
            ("not(windows)", Truth::Yes),
            ("not(docsrs)", Truth::Unknown),
            ("all(unix, feature = \"simd\")", Truth::No),
            ("all(unix, docsrs)", Truth::Unknown),
            ("all(windows, docsrs)", Truth::No),
            ("any(windows, docsrs)", Truth::Unknown),
            ("any(unix, docsrs)", Truth::Yes),
            ("any(windows, target_os = \"linux\")", Truth::No),
            ("all()", Truth::Yes),
            ("any()", Truth::No),
            ("all(unix,)", Truth::Yes),
        ] {
            assert_eq!(cfg.holds(predicate), truth, "{predicate}");
        }
    }

    #[test]
    fn nothing_is_left_out_when_nothing_is_known() {
        let cfg = Cfg::unknown();
        for predicate in ["windows", "feature = \"simd\"", "not(unix)"] {
            assert_eq!(cfg.holds(predicate), Truth::Unknown, "{predicate}");
        }
        let no_target = Cfg::new("", BTreeSet::new());
        assert_eq!(no_target.holds("windows"), Truth::Unknown);
        assert_eq!(no_target.holds("feature = \"simd\""), Truth::No);
    }

    #[test]
    fn this_machine_is_asked() {
        let here = Cfg::here(BTreeSet::new());
        assert_eq!(here.holds("all(unix, windows)"), Truth::No);
        assert_eq!(here.holds("any(unix, windows)"), Truth::Yes);
    }

    #[test]
    fn the_builds_own_cfgs_leave_out_every_name_it_doesnt_have() {
        let cfg = Cfg::built(&format!("{MAC}from_build\nfeature=\"std\"\n"));
        for (predicate, truth) in [
            ("unix", Truth::Yes),
            ("windows", Truth::No),
            ("from_build", Truth::Yes),
            ("not(from_build)", Truth::No),
            ("docsrs", Truth::No),
            ("not(docsrs)", Truth::Yes),
            ("loom", Truth::No),
            ("fuzzing", Truth::No),
            ("note = \"x\"", Truth::No),
            ("feature = \"std\"", Truth::Yes),
            ("feature = \"simd\"", Truth::No),
            ("miri", Truth::No),
            ("debug_assertions", Truth::Yes),
            ("true", Truth::Yes),
            ("false", Truth::No),
            ("test", Truth::Unknown),
            ("not(test)", Truth::Unknown),
            ("any(test, loom)", Truth::Unknown),
            ("all(test, loom)", Truth::No),
            ("all(unix, not(loom))", Truth::Yes),
        ] {
            assert_eq!(cfg.holds(predicate), truth, "{predicate}");
        }
        assert_eq!(Cfg::built("").holds("windows"), Truth::Unknown);
    }

    #[test]
    fn what_decides_the_cfgs_is_kept_from_a_compile() {
        let args: Vec<OsString> = [
            "--crate-name",
            "shop",
            "--edition=2021",
            "src/lib.rs",
            "--cfg",
            "feature=\"std\"",
            "--cfg=from_build",
            "-C",
            "incremental=/tmp/x",
            "-C",
            "debug-assertions=on",
            "-Ctarget-cpu=native",
            "--codegen=panic=abort",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--check-cfg",
            "cfg(docsrs)",
            "-L",
            "dependency=/tmp/deps",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        let kept: Vec<String> = deciding(&args)
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            kept,
            [
                "--cfg",
                "feature=\"std\"",
                "--cfg=from_build",
                "-C",
                "debug-assertions=on",
                "-Ctarget-cpu=native",
                "--codegen=panic=abort",
                "--target",
                "x86_64-unknown-linux-gnu",
            ]
        );
    }

    #[test]
    fn default_features_follow_what_they_enable() {
        let table = serde_json::json!({
            "default": ["std", "dep:log", "serde?/std"],
            "std": ["alloc", "regex/unicode"],
            "alloc": [],
            "regex": ["dep:regex"],
            "simd": [],
            "serde": ["dep:serde"],
        });
        let on: Vec<String> = default_features(&table).into_iter().collect();
        assert_eq!(on, ["alloc", "default", "regex", "std"]);
        assert!(default_features(&serde_json::json!({ "simd": [] })).is_empty());
    }
}
