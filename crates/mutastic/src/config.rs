//! `mutastic.yaml`, the nearest at or above the package's directory: the project's own say over
//! which mutants run, in the Java edition's format, read as strictly: a line it doesn't understand
//! is an error that names it, never a guess.
//!
//! ```yaml
//! extends: ../mutastic.yaml
//! rules:
//!   - skip: shop::generated::**
//!     reason: protobuf output
//! plugins:
//!   - rules/strings.yaml
//! operators:
//!   - stronger
//!   - -stream/swap-match
//! gate:
//!   operators:
//!     - stronger
//! ```
//!
//! Patterns name Rust paths where the Java edition names classes: `**` matches any depth, `*` one
//! name, and a function is its path, `shop::Money::add`, where Java writes `com.acme.Money#add`.
//! A plugin is a rules file of call operators; the Rust edition has no compiled plugins yet, so
//! `filters:`, which names a compiled plugin's filters, is an error. `gate:`'s `operators:` chooses
//! for `gate open` alone, in place of `operators:`, so an agent's task can be held to more than
//! everyday runs.

use std::path::{Path, PathBuf};

use crate::failure::{Code, Failure};

pub const FILE: &str = "mutastic.yaml";

/// A project's decision: skip the mutants in some code, mutate code a filter would skip, or name
/// calls of its own that only write output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rule {
    pub mutate: bool,
    /// An `output:` rule: calls of the functions or macros `target` names only write output, as
    /// a logger's do, and are skipped as `mutastic:output` with the rule's reason.
    pub output: bool,
    pub target: String,
    pub reason: String,
    /// A `mutate:` rule's `arid:` list: the ids of the arid rules it brings back
    /// (`mutastic:output`); empty for every arid skip, and for a `skip:` rule.
    pub arid: Vec<String>,
}

impl Rule {
    /// Whether the rule covers the function at `path` (`shop::Money::add`), or the item holding it
    /// (`shop::Money`), as a Java rule covers a method or its class.
    pub fn covers(&self, path: &str) -> bool {
        glob(&self.target, path)
            || path
                .rsplit_once("::")
                .is_some_and(|(holder, _)| glob(&self.target, holder))
    }

    /// `skip: shop::generated::**`, as the rule was written.
    pub fn written(&self) -> String {
        format!("{}: {}", verb(self.mutate, self.output), self.target)
    }

    /// Whether it's a `skip:` rule.
    pub fn skips(&self) -> bool {
        !self.mutate && !self.output
    }

    /// Whether an `output:` rule names the function or macro a statement calls, by its path as
    /// written (`audit::record`, `counter`): the path's segments are the last of the rule's, each
    /// matched as a name in it is, or the whole path matches it.
    pub fn names_call(&self, path: &str) -> bool {
        if !self.output || path.is_empty() {
            return false;
        }
        if glob(&self.target, path) || glob(&self.target, &format!("crate::{path}")) {
            return true;
        }
        let target: Vec<&str> = self.target.split("::").collect();
        let called: Vec<&str> = path.split("::").collect();
        called.len() <= target.len()
            && !self.target.contains("**")
            && target[target.len() - called.len()..]
                .iter()
                .zip(&called)
                .all(|(pattern, name)| glob(pattern, name))
    }

    /// `arid: only calls audit::record (the audit trail)`: the skip of a call an `output:` rule
    /// names.
    pub fn output_reason(&self, path: &str) -> String {
        format!("arid: only calls {path} ({})", self.reason)
    }

    /// Whether a `mutate:` rule brings back a mutant skipped for `reason` (`arid: only writes
    /// output`): an arid skip, of one of the rules its `arid:` lists if it has one.
    pub fn brings_back(&self, reason: &str) -> bool {
        // An `output:` rule's skips are arid, and a `mutate:` rule brings them back.
        self.mutate
            && reason.starts_with("arid:")
            && (self.arid.is_empty()
                || crate::arid::rule(reason)
                    .is_some_and(|rule| self.arid.iter().any(|id| id == rule.id)))
    }

    /// `user: protobuf output (rule `skip: shop::generated::**`)`: the skip reason a mutant gets.
    pub fn skip_reason(&self) -> String {
        format!("user: {} (rule `{}`)", self.reason, self.written())
    }
}

/// `**` matches across `::`, `*` within one name.
/// What a rule starts with: `skip`, `mutate` or `output`.
fn verb(mutate: bool, output: bool) -> &'static str {
    match (mutate, output) {
        (true, _) => "mutate",
        (false, true) => "output",
        (false, false) => "skip",
    }
}

fn glob(pattern: &str, text: &str) -> bool {
    fn matches(pattern: &[u8], text: &[u8]) -> bool {
        match pattern {
            [] => text.is_empty(),
            [b'*', b'*', rest @ ..] => (0..=text.len()).any(|at| matches(rest, &text[at..])),
            [b'*', rest @ ..] => (0..=text.len())
                .take_while(|&at| at == 0 || text[at - 1] != b':')
                .any(|at| matches(rest, &text[at..])),
            [first, rest @ ..] => text.first() == Some(first) && matches(rest, &text[1..]),
        }
    }
    matches(pattern.as_bytes(), text.as_bytes())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Configuration {
    pub rules: Vec<Rule>,
    /// Rules files, each found from the file that names it: once read, the path from where the
    /// first file was read.
    pub plugins: Vec<PathBuf>,
    /// Where each of `plugins` is named, `mutastic.yaml line 4`, for an error about it.
    pub plugins_from: Vec<String>,
    pub operators: Vec<String>,
    /// Where each of `operators` is written, `mutastic.yaml line 4`, for an error about it.
    pub operators_from: Vec<String>,
    /// The operators `gate:` chooses for a gated task: empty when it chooses none, and the gate
    /// runs `operators`.
    pub gate_operators: Vec<String>,
    /// Where each of `gate_operators` is written.
    pub gate_operators_from: Vec<String>,
    /// The tests `exclude-tests:` leaves out of every run, each with its reason.
    pub exclude_tests: Vec<ExcludedTest>,
    /// `timeout-ceiling:`, the most one mutant's tests may take in all: none when the file sets
    /// none, and the suite's own time sets it ([`crate::limits`]).
    pub timeout_ceiling: Option<std::time::Duration>,
}

/// A test `exclude-tests:` leaves out: a test that fails here on its own, until it's fixed, by
/// its name as `--exclude-tests` takes it, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExcludedTest {
    pub test: String,
    pub reason: String,
}

/// Two entries for one test are one: the first file to name it gives the reason.
fn joined_tests(first: Vec<ExcludedTest>, then: Vec<ExcludedTest>) -> Vec<ExcludedTest> {
    let mut both = first;
    for each in then {
        if !both.iter().any(|kept| kept.test == each.test) {
            both.push(each);
        }
    }
    both
}

impl Configuration {
    /// The configuration in `dir`, or none when it has no `mutastic.yaml`: the file's own, then
    /// what it `extends:`, in turn. A file adds another's only when it says so, so one file and the
    /// ones it names are all that apply, never a cascade nobody wrote down.
    pub fn read(dir: &Path) -> Result<Configuration, Failure> {
        Self::read_chain(dir).map_err(|error| Code::Config.with(error))
    }

    fn read_chain(dir: &Path) -> Result<Configuration, String> {
        let base = normalized(dir);
        let first = base.join(FILE);
        if !first.is_file() {
            return Ok(Configuration::default());
        }
        // Named with `/` on every platform, as the file's own `extends:` lines name them.
        let shown = |file: &Path| {
            let names: Vec<String> = relative(&base, file)
                .components()
                .map(|name| name.as_os_str().to_string_lossy().into_owned())
                .collect();
            names.join("/")
        };
        // The files read so far, in order, so one that comes back round is an error, not a loop.
        let mut chain = vec![first.clone()];
        let mut file = first;
        let mut config = Configuration::default();
        loop {
            let text = std::fs::read_to_string(&file)
                .map_err(|error| format!("{}: {error}", shown(&file)))?;
            let written = parse_as(&text, &shown(&file))?;
            let from = file.parent().map(Path::to_path_buf).unwrap_or_default();
            config = config.over(written.own.found_from(&from));
            let Some(extends) = written.extends else {
                return Ok(config);
            };
            let named = normalized(&from.join(&extends.path));
            if !named.is_file() {
                return Err(format!(
                    "{} line {}: extends {}, which isn't there",
                    shown(&file),
                    extends.line,
                    extends.path
                ));
            }
            if chain.contains(&named) {
                let names: Vec<String> = chain.iter().chain([&named]).map(|f| shown(f)).collect();
                return Err(format!(
                    "extends: goes round in a loop: {}",
                    names.join(" extends ")
                ));
            }
            chain.push(named.clone());
            file = named;
        }
    }

    /// This file's say over `parent`'s: its rules first, so a package's own reason is the one
    /// shown, then the parent's; the plugins of both; and its operators, and its gate's, if it
    /// chooses any, in place of the parent's, as `--operators` replaces them.
    fn over(self, parent: Configuration) -> Configuration {
        let named = |plugins: Vec<PathBuf>, from: Vec<String>| -> Vec<Named> {
            plugins
                .into_iter()
                .zip(from)
                .map(|(p, f)| Named(p, f))
                .collect()
        };
        let plugins = joined(
            named(self.plugins, self.plugins_from),
            named(parent.plugins, parent.plugins_from),
        );
        let (plugins, plugins_from) = plugins.into_iter().map(|Named(p, f)| (p, f)).unzip();
        Configuration {
            rules: joined(self.rules, parent.rules),
            exclude_tests: joined_tests(self.exclude_tests, parent.exclude_tests),
            plugins,
            plugins_from,
            operators_from: if self.operators.is_empty() {
                parent.operators_from
            } else {
                self.operators_from
            },
            operators: if self.operators.is_empty() {
                parent.operators
            } else {
                self.operators
            },
            gate_operators_from: if self.gate_operators.is_empty() {
                parent.gate_operators_from
            } else {
                self.gate_operators_from
            },
            gate_operators: if self.gate_operators.is_empty() {
                parent.gate_operators
            } else {
                self.gate_operators
            },
            timeout_ceiling: self.timeout_ceiling.or(parent.timeout_ceiling),
        }
    }

    /// The same configuration, its plugins found from `dir`, the directory of the file that
    /// names them.
    fn found_from(self, dir: &Path) -> Configuration {
        let plugins = self
            .plugins
            .iter()
            .map(|plugin| normalized(&dir.join(plugin)))
            .collect();
        Configuration { plugins, ..self }
    }
}

/// `first` then `then`, each once.
fn joined<T: PartialEq>(first: Vec<T>, then: Vec<T>) -> Vec<T> {
    let mut both = first;
    for each in then {
        if !both.contains(&each) {
            both.push(each);
        }
    }
    both
}

/// A plugin, and where it's named.
struct Named(PathBuf, String);

/// A plugin named twice, by two files, is one plugin: the first file to name it says where.
impl PartialEq for Named {
    fn eq(&self, other: &Named) -> bool {
        self.0 == other.0
    }
}

/// Where a package's `mutastic.yaml` is: the nearest at or above the package's directory, no
/// higher than the top of its git repository, or, outside one, than the directory the run is on,
/// so a stray file above the project is never read. Both are absolute. The same package gets the
/// same rules from the workspace's top as from its own directory.
pub fn nearest(package: &Path, run: &Path) -> Option<PathBuf> {
    let top = package
        .ancestors()
        .find(|at| at.join(".git").exists())
        .unwrap_or(run);
    package
        .ancestors()
        .take_while(|at| at.starts_with(top))
        .find(|at| at.join(FILE).is_file())
        .map(Path::to_path_buf)
}

/// The path without `.` and with each `..` taking away the name before it, as Java's `normalize`
/// does: a file named two ways is one file.
fn normalized(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir
                if matches!(out.components().next_back(), Some(Component::Normal(_))) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The path to `to` from the directory `from`, both normalized: `../mutastic.yaml`.
pub fn relative(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut path: PathBuf = from[shared..].iter().map(|_| "..").collect();
    path.extend(&to[shared..]);
    path
}

/// One file as written: its own say, and the file it `extends:`, if any.
struct Written {
    own: Configuration,
    extends: Option<Extends>,
}

/// `extends: <path>`, on its line.
struct Extends {
    line: usize,
    path: String,
}

/// Reads the file a line at a time, in the shape above, as `mutastic.yaml` in the directory it's
/// read from.
pub fn parse(text: &str) -> Result<Configuration, String> {
    parse_as(text, FILE).map(|written| written.own)
}

/// Reads a file that its errors name as `shown`.
fn parse_as(text: &str, shown: &str) -> Result<Written, String> {
    let mut config = Configuration::default();
    let mut extends: Option<Extends> = None;
    let mut section = "";
    // Under `gate:`, whether its `operators:` has been read.
    let mut gate_key = false;
    let mut pending: Option<Pending> = None;
    // The test under `exclude-tests:` being read, and its reason once read.
    let mut pending_test: Option<(String, Option<String>)> = None;
    let mut last = 0;
    for (index, raw) in text.lines().enumerate() {
        let error = |what: &str| format!("{shown} line {}: {what}", index + 1);
        let line = without_comment(raw);
        if line.trim().is_empty() {
            continue;
        }
        last = index + 1;
        if !line.starts_with(' ') {
            if let Some(rule) = pending.take() {
                config.rules.push(rule.done().map_err(|what| error(&what))?);
            }
            if let Some(test) = pending_test.take() {
                config
                    .exclude_tests
                    .push(test_done(test).map_err(|what| error(&what))?);
            }
            if let Some(path) = line.trim().strip_prefix("extends:") {
                if extends.is_some() {
                    return Err(error("a file extends one other file, once"));
                }
                let path = value(path);
                if path.is_empty() {
                    return Err(error("`extends:` needs a path, such as ../mutastic.yaml"));
                }
                extends = Some(Extends {
                    line: index + 1,
                    path,
                });
                section = "";
                continue;
            }
            if let Some(ceiling) = line.trim().strip_prefix("timeout-ceiling:") {
                if config.timeout_ceiling.is_some() {
                    return Err(error("a file sets `timeout-ceiling:` once"));
                }
                let ceiling = crate::limits::parse_ceiling(&value(ceiling));
                config.timeout_ceiling = Some(ceiling.map_err(|what| error(&what))?);
                section = "";
                continue;
            }
            section = match line.trim() {
                "rules:" => "rules",
                "plugins:" => "plugins",
                "filters:" => "filters",
                "operators:" => "operators",
                "exclude-tests:" => "exclude-tests",
                "gate:" => "gate",
                key if key.starts_with("gate:") => return Err(error(GATE_SHAPE)),
                line => return Err(error(&not_a_section(line))),
            };
            continue;
        }
        let item = line.trim();
        if section == "rules" {
            let indent = line.len() - line.trim_start().len();
            if let Some(rule) = pending.as_mut() {
                if rule.arid_indent.is_some_and(|under| indent > under) && item.starts_with("- ") {
                    rule.arid_id(&value(&item[2..]))
                        .map_err(|what| error(&what))?;
                    continue;
                }
                rule.arid_indent = None;
                if let Some(reason) = item.strip_prefix("reason:") {
                    if rule.reason.is_some() {
                        return Err(error(&format!(
                            "the rule `{}` has one `reason:`",
                            rule.written()
                        )));
                    }
                    rule.reason = Some(value(reason));
                    continue;
                }
                if let Some(arid) = item.strip_prefix("arid:") {
                    rule.arid(&value(arid), indent)
                        .map_err(|what| error(&what))?;
                    continue;
                }
            }
            let listed = listed(item).ok_or_else(|| error("expected a list item, `- …`"))?;
            let (verb, target) = listed.split_once(':').unwrap_or(("", ""));
            if verb != "skip" && verb != "mutate" && verb != "output" {
                if let Some(rule) = crate::similar::closest(verb, ["skip", "mutate", "output"]) {
                    return Err(error(&format!(
                        "{verb}: isn't a rule. There's one called {rule}:."
                    )));
                }
                return Err(error(
                    "a rule is `- skip: <pattern>`, `- mutate: <pattern>` or `- output: <pattern>`, then `reason: …`",
                ));
            }
            if let Some(rule) = pending.take() {
                config.rules.push(rule.done().map_err(|what| error(&what))?);
            }
            pending = Some(Pending {
                mutate: verb == "mutate",
                output: verb == "output",
                target: value(target),
                ..Pending::default()
            });
            continue;
        }
        if section == "exclude-tests" {
            if let Some((test, reason)) = pending_test.as_mut() {
                if let Some(why) = item.strip_prefix("reason:") {
                    if reason.is_some() {
                        return Err(error(&format!("the test `{test}` has one `reason:`")));
                    }
                    *reason = Some(value(why));
                    continue;
                }
            }
            let test = listed(item)
                .and_then(|listed| listed.strip_prefix("test:").map(value))
                .filter(|test| !test.is_empty())
                .ok_or_else(|| error(TEST_SHAPE))?;
            if let Some(done) = pending_test.take() {
                config
                    .exclude_tests
                    .push(test_done(done).map_err(|what| error(&what))?);
            }
            pending_test = Some((test, None));
            continue;
        }
        if section == "gate" {
            if item == "operators:" && !gate_key {
                gate_key = true;
                continue;
            }
            let listed = listed(item)
                .filter(|_| gate_key)
                .ok_or_else(|| error(GATE_SHAPE))?;
            config.gate_operators.push(listed);
            config
                .gate_operators_from
                .push(format!("{shown} line {}", index + 1));
            continue;
        }
        let listed = listed(item).ok_or_else(|| error("expected a list item, `- …`"))?;
        match section {
            "plugins" => {
                config.plugins.push(PathBuf::from(listed));
                config.plugins_from.push(format!("{shown} line {}", index + 1));
            }
            "operators" => {
                config.operators.push(listed);
                config.operators_from.push(format!("{shown} line {}", index + 1));
            }
            "filters" => {
                return Err(error(&format!(
                    "the filter {listed}: filters come from compiled plugins, which the Rust edition doesn't have yet"
                )))
            }
            _ => return Err(error("a list item needs a section above it")),
        }
    }
    if let Some(rule) = pending.take() {
        config.rules.push(
            rule.done()
                .map_err(|what| format!("{shown} line {last}: {what}"))?,
        );
    }
    if let Some(test) = pending_test.take() {
        config
            .exclude_tests
            .push(test_done(test).map_err(|what| format!("{shown} line {last}: {what}"))?);
    }
    Ok(Written {
        own: config,
        extends,
    })
}

/// What an entry under `exclude-tests:` is, as an error says it.
const TEST_SHAPE: &str = "a test to leave out is `- test: <name>`, then `reason: …`";

/// A test to leave out, once it has its reason: a test left out says why, in the summary, so one
/// without a reason is an error.
fn test_done((test, reason): (String, Option<String>)) -> Result<ExcludedTest, String> {
    match reason.filter(|reason| !reason.is_empty()) {
        Some(reason) => Ok(ExcludedTest { test, reason }),
        None => Err(format!(
            "the test `{test}` needs a `reason:` under it: why it's left out, which the summary \
             shows"
        )),
    }
}

/// A rule being read: its verb and pattern, and what's under it so far.
#[derive(Default)]
struct Pending {
    mutate: bool,
    output: bool,
    target: String,
    reason: Option<String>,
    arid: Vec<String>,
    arid_given: bool,
    /// The indent of an `arid:` with its list under it, while that list is read.
    arid_indent: Option<usize>,
}

impl Pending {
    fn written(&self) -> String {
        format!("{}: {}", verb(self.mutate, self.output), self.target)
    }

    /// A `mutate:` rule's `arid:`: a list on its line, `[mutastic:output]`, one id, or nothing,
    /// and its list on the lines under it.
    fn arid(&mut self, value: &str, indent: usize) -> Result<(), String> {
        if !self.mutate {
            return Err(format!(
                "`arid:` goes on a `mutate:` rule: it says which arid skips to bring back, and {}",
                if self.output {
                    "an `output:` rule's calls are all skipped as `mutastic:output`"
                } else {
                    "a `skip:` rule skips everything it covers"
                }
            ));
        }
        if self.arid_given {
            return Err(format!(
                "the rule `{}` has one `arid:` list",
                self.written()
            ));
        }
        self.arid_given = true;
        if value.is_empty() {
            self.arid_indent = Some(indent);
            return Ok(());
        }
        let items = value
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .unwrap_or(value);
        if items.trim().is_empty() {
            return Ok(());
        }
        for id in items.split(',') {
            self.arid_id(&self::value(id))?;
        }
        Ok(())
    }

    /// One id in an `arid:` list, which must be one of mutastic's arid rules.
    fn arid_id(&mut self, id: &str) -> Result<(), String> {
        let ids = crate::arid::ids();
        if !ids.contains(&id) {
            return Err(match crate::similar::closest(id, ids.iter().copied()) {
                Some(close) => format!("{id} isn't an arid rule. There's one called {close}."),
                None => format!(
                    "{} isn't an arid rule; the rules are {}",
                    if id.is_empty() { "\"\"" } else { id },
                    match ids.split_last() {
                        Some((last, rest)) if !rest.is_empty() =>
                            format!("{} and {last}", rest.join(", ")),
                        _ => ids.join(""),
                    }
                ),
            });
        }
        self.arid.push(id.to_string());
        Ok(())
    }

    /// The rule, once it has its reason: every skip says why, so a rule without one is an error,
    /// not "no reason given".
    fn done(self) -> Result<Rule, String> {
        let written = self.written();
        let Some(reason) = self.reason else {
            return Err(format!("the rule `{written}` needs a `reason:` under it"));
        };
        if self.arid_given && self.arid.is_empty() {
            return Err(format!(
                "the rule `{written}` has an empty `arid:` list: name the rules to bring back, or \
                 leave `arid:` out to bring back every arid skip"
            ));
        }
        Ok(Rule {
            mutate: self.mutate,
            output: self.output,
            target: self.target,
            reason,
            arid: self.arid,
        })
    }
}

fn listed(item: &str) -> Option<String> {
    item.strip_prefix("- ").map(value)
}

/// A value, without the quotes around it.
pub fn value(text: &str) -> String {
    let value = text.trim();
    let quoted = value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')));
    if quoted {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

/// A `#` at the start or after a space starts a comment, as in YAML.
pub fn without_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for at in 0..bytes.len() {
        if bytes[at] == b'#' && (at == 0 || bytes[at - 1] == b' ') {
            return &line[..at];
        }
    }
    line
}

/// The operators `entries` choose, as `--operators` and `operators:` read them: a set
/// (`defaults`, `stronger`, `extended`, `all`), a namespace (`stream`) or an operator's id, each
/// adding to the ones before, or after `-` taking away; a rules file's operators count too.
pub fn resolve(entries: &[String], from: &str) -> Result<Vec<&'static str>, Failure> {
    let from = vec![from.to_string(); entries.len()];
    resolve_from(entries, &from)
}

/// The operators `entries` choose, each said, in an error, to be from its own place in `from`.
pub fn resolve_from(entries: &[String], from: &[String]) -> Result<Vec<&'static str>, Failure> {
    use crate::sites;
    let rules = crate::callrules::loaded();
    let mut chosen: Vec<&'static str> = Vec::new();
    // A list that only leaves operators out leaves them out of the defaults: `-stream`.
    if entries.iter().all(|entry| entry.starts_with('-')) {
        chosen.extend(sites::DEFAULTS.iter().copied());
        chosen.extend(rules.iter().filter(|r| r.in_tier("defaults")).map(|r| r.id));
    }
    for (index, entry) in entries.iter().enumerate() {
        let (remove, name) = match entry.strip_prefix('-') {
            Some(name) => (true, name),
            None => (false, entry.as_str()),
        };
        let matched: Vec<&'static str> = match name {
            "defaults" | "stronger" | "extended" | "all" => {
                let set = match name {
                    "defaults" => sites::DEFAULTS,
                    "stronger" => sites::STRONGER,
                    _ => sites::ALL,
                };
                let tier = if name == "all" { "extended" } else { name };
                set.iter()
                    .copied()
                    .chain(rules.iter().filter(|r| r.in_tier(tier)).map(|r| r.id))
                    .collect()
            }
            _ => sites::ALL
                .iter()
                .copied()
                .chain(rules.iter().map(|r| r.id))
                .filter(|id| {
                    *id == name || (!name.contains('/') && id.starts_with(&format!("{name}/")))
                })
                .collect(),
        };
        if matched.is_empty() {
            let from = from.get(index).map_or("", String::as_str);
            return Err(Code::Config.with(no_operator(from, name)));
        }
        if remove {
            chosen.retain(|id| !matched.contains(id));
        } else {
            for id in matched {
                if !chosen.contains(&id) {
                    chosen.push(id);
                }
            }
        }
    }
    Ok(chosen)
}

/// What a line at the left that isn't a section is told, as the Java edition tells it: a list on
/// the section's own line, which goes a line under it; a close name; or the sections there are.
fn not_a_section(line: &str) -> String {
    let key = line.find(':').map_or(line, |colon| &line[..=colon]);
    let bare = key.trim_end_matches(':');
    if SECTIONS.contains(&bare) {
        let item = line[key.len()..]
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']');
        return format!("{key} takes a list, an item a line under it:\n  {key}\n    - {item}");
    }
    match crate::similar::closest(bare, SECTIONS.iter().copied()) {
        Some(close) => format!("{key} isn't a section. There's one called {close}:."),
        None => format!(
            "{key} isn't a section; the sections are extends:, rules:, plugins:, filters:, \
             operators:, exclude-tests:, gate: and timeout-ceiling:"
        ),
    }
}

/// The sections a `mutastic.yaml` has, without their colons.
const SECTIONS: &[&str] = &[
    "extends",
    "rules",
    "plugins",
    "filters",
    "operators",
    "exclude-tests",
    "gate",
    "timeout-ceiling",
];

/// What `gate:` takes, as an error says it.
const GATE_SHAPE: &str =
    "gate: takes operators:, a list under it:\n  gate:\n    operators:\n      - stronger";

/// `name` is no set, namespace or operator mutastic has: what `--operators` and `operators:` take,
/// and a close name.
fn no_operator(from: &str, name: &str) -> String {
    let rules = crate::callrules::loaded();
    let ids: Vec<&str> = crate::sites::ALL
        .iter()
        .copied()
        .chain(rules.iter().map(|rule| rule.id))
        .collect();
    let mut namespaces: Vec<&str> = ids
        .iter()
        .filter_map(|id| id.split_once('/').map(|(space, _)| space))
        .collect();
    namespaces.sort_unstable();
    namespaces.dedup();
    let sets = ["defaults", "stronger", "extended", "all"];
    let names = sets.iter().chain(&namespaces).chain(&ids).copied();
    format!(
        "{from}: {name} isn't an operator mutastic has.{}\n\
         It takes a set ({}), a namespace ({}) or an operator's id. To see each id:\n  \
         mutastic mutants --operators=all",
        crate::similar::offered(name, names),
        crate::mistyped::listed(&sets),
        crate::mistyped::listed(&namespaces),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_java_editions_shape_is_read_with_rust_paths() {
        let config = parse(
            "# the project's say\nrules:\n  - skip: shop::generated::**\n    reason: protobuf output\n\
             \x20 - mutate: 'shop::Money::fmt'\n    reason: shown to customers # why\n\
             plugins:\n  - rules/strings.yaml\noperators:\n  - stronger\n  - -stream/swap-match\n",
        )
        .unwrap();
        assert_eq!(config.rules.len(), 2);
        assert_eq!(
            config.rules[0].skip_reason(),
            "user: protobuf output (rule `skip: shop::generated::**`)"
        );
        assert!(config.rules[1].mutate);
        assert_eq!(config.plugins, [PathBuf::from("rules/strings.yaml")]);
        assert_eq!(config.operators, ["stronger", "-stream/swap-match"]);
    }

    /// An `output:` rule names the project's own output calls, with its reason.
    #[test]
    fn an_output_rule_names_the_projects_own_output_calls() {
        let config = parse(
            "rules:\n  - output: crate::audit::record\n    reason: the audit trail\n\
             \x20 - output: metrics::counter\n    reason: a metric\n",
        )
        .unwrap();
        assert_eq!(config.rules.len(), 2);
        assert!(config.rules.iter().all(|rule| rule.output && !rule.skips()));
        assert_eq!(config.rules[0].written(), "output: crate::audit::record");
        assert!(config.rules[0].names_call("audit::record"));
        assert!(config.rules[0].names_call("record"));
        assert!(config.rules[0].names_call("crate::audit::record"));
        assert!(!config.rules[0].names_call("audit::keep"));
        assert!(!config.rules[0].names_call("other::audit::record"));
        assert!(config.rules[1].names_call("counter"));
        assert_eq!(
            config.rules[1].output_reason("metrics::counter"),
            "arid: only calls metrics::counter (a metric)"
        );
    }

    #[test]
    fn what_it_doesnt_understand_is_an_error_that_names_the_line() {
        for (text, error) in [
            ("rule:\n", "mutastic.yaml line 1: rule: isn't a section. There's one called rules:."),
            ("colour:\n", "mutastic.yaml line 1: colour: isn't a section; the sections are extends:, rules:, plugins:, filters:, operators:, exclude-tests:, gate: and timeout-ceiling:"),
            ("operators: [stronger]\n", "mutastic.yaml line 1: operators: takes a list, an item a line under it:\n  operators:\n    - stronger"),
            ("rules:\n  - skp: a\n", "mutastic.yaml line 2: skp: isn't a rule. There's one called skip:."),
            ("extends: a.yaml\nextends: b.yaml\n", "mutastic.yaml line 2: a file extends one other file, once"),
            ("extends: ''\n", "mutastic.yaml line 1: `extends:` needs a path, such as ../mutastic.yaml"),
            ("extends: a.yaml\n  - x\n", "mutastic.yaml line 2: a list item needs a section above it"),
            ("rules:\n  - skip: a::**\n", "mutastic.yaml line 2: the rule `skip: a::**` needs a `reason:` under it"),
            ("rules:\n  - drop: a\n", "mutastic.yaml line 2: a rule is `- skip: <pattern>`, `- mutate: <pattern>` or `- output: <pattern>`, then `reason: …`"),
            ("rules:\n  - output: audit::record\n", "mutastic.yaml line 2: the rule `output: audit::record` needs a `reason:` under it"),
            ("rules:\n  - output: a\n    arid: [mutastic:output]\n    reason: r\n", "mutastic.yaml line 3: `arid:` goes on a `mutate:` rule: it says which arid skips to bring back, and an `output:` rule's calls are all skipped as `mutastic:output`"),
            ("operators:\n  stronger\n", "mutastic.yaml line 2: expected a list item, `- …`"),
            ("rules:\n  - mutate: a\n    arid: [mutastic:outptu]\n    reason: r\n", "mutastic.yaml line 3: mutastic:outptu isn't an arid rule. There's one called mutastic:output."),
            ("rules:\n  - mutate: a\n    arid: [sleeps]\n    reason: r\n", "mutastic.yaml line 3: sleeps isn't an arid rule; the rules are mutastic:debug-representations, mutastic:output, mutastic:messages, mutastic:capacity, mutastic:wait-times, mutastic:waits, mutastic:null-checks, mutastic:thread-names, mutastic:keep-alive and mutastic:stack-sizes"),
            ("rules:\n  - skip: a\n    arid: [mutastic:output]\n    reason: r\n", "mutastic.yaml line 3: `arid:` goes on a `mutate:` rule: it says which arid skips to bring back, and a `skip:` rule skips everything it covers"),
            ("exclude-tests:\n  - test: tests::races\n", "mutastic.yaml line 2: the test `tests::races` needs a `reason:` under it: why it's left out, which the summary shows"),
            ("exclude-tests:\n  - test: a\n    reason: ''\noperators:\n  - stronger\n", "mutastic.yaml line 4: the test `a` needs a `reason:` under it: why it's left out, which the summary shows"),
            ("exclude-tests:\n  - test: a\n  - test: b\n    reason: r\n", "mutastic.yaml line 3: the test `a` needs a `reason:` under it: why it's left out, which the summary shows"),
            ("exclude-tests:\n  - test: a\n    reason: r\n    reason: s\n", "mutastic.yaml line 4: the test `a` has one `reason:`"),
            ("exclude-tests:\n  - tests::races\n", "mutastic.yaml line 2: a test to leave out is `- test: <name>`, then `reason: …`"),
            ("exclude-tests:\n  reason: r\n", "mutastic.yaml line 2: a test to leave out is `- test: <name>`, then `reason: …`"),
            ("exclude-tests: [a]\n", "mutastic.yaml line 1: exclude-tests: takes a list, an item a line under it:\n  exclude-tests:\n    - a"),
            ("exclude-test:\n", "mutastic.yaml line 1: exclude-test: isn't a section. There's one called exclude-tests:."),
            ("rules:\n  - mutate: a\n    arid: []\n    reason: r\n", "mutastic.yaml line 4: the rule `mutate: a` has an empty `arid:` list: name the rules to bring back, or leave `arid:` out to bring back every arid skip"),
            ("timeout-ceiling: 10\n", "mutastic.yaml line 1: `timeout-ceiling:` takes a time above 0 with its unit, such as 10m, 90s or 2h, not 10"),
            ("timeout-ceiling: 10m\ntimeout-ceiling: 5m\n", "mutastic.yaml line 2: a file sets `timeout-ceiling:` once"),
            ("timeout_ceiling: 10m\n", "mutastic.yaml line 1: timeout_ceiling: isn't a section. There's one called timeout-ceiling:."),
        ] {
            assert_eq!(parse(text).unwrap_err(), error);
        }
    }

    /// A `mutate:` rule's `arid:` names the arid rules it brings back, on its line or under it,
    /// before or after its reason, as in the Java edition.
    #[test]
    fn a_mutate_rule_names_the_arid_rules_it_brings_back() {
        let config = parse(
            "rules:\n  - mutate: shop::log\n    arid: [mutastic:output, \"mutastic:null-checks\"]\n    reason: read\n\
             \x20 - mutate: shop::check\n    reason: checked\n    arid: mutastic:null-checks\n\
             \x20 - mutate: shop::both\n    arid:\n      - mutastic:output\n    reason: both\n\
             \x20 - mutate: shop::all\n    reason: all\n",
        )
        .unwrap();
        let arid: Vec<(&str, Vec<&str>)> = config
            .rules
            .iter()
            .map(|rule| {
                (
                    rule.target.as_str(),
                    rule.arid.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            arid,
            [
                ("shop::log", vec!["mutastic:output", "mutastic:null-checks"]),
                ("shop::check", vec!["mutastic:null-checks"]),
                ("shop::both", vec!["mutastic:output"]),
                ("shop::all", vec![]),
            ]
        );
    }

    /// `timeout-ceiling:` sets the most one mutant's tests may take, quoted or not, and a file's
    /// own is kept over the one it extends.
    #[test]
    fn timeout_ceiling_sets_the_most_a_mutants_tests_take() {
        let minutes = |m: u64| Some(std::time::Duration::from_secs(m * 60));
        assert_eq!(
            parse("timeout-ceiling: 30m\n").unwrap().timeout_ceiling,
            minutes(30)
        );
        assert_eq!(
            parse("operators:\n  - stronger\ntimeout-ceiling: '2h' # a long suite\n")
                .unwrap()
                .timeout_ceiling,
            minutes(120)
        );
        assert_eq!(
            parse("operators:\n  - stronger\n").unwrap().timeout_ceiling,
            None
        );
        let dir = written(
            "ceiling",
            &[
                ("mutastic.yaml", "timeout-ceiling: 30m\n"),
                (
                    "own/mutastic.yaml",
                    "extends: ../mutastic.yaml\ntimeout-ceiling: 5m\n",
                ),
                ("kept/mutastic.yaml", "extends: ../mutastic.yaml\n"),
            ],
        );
        let read = |package: &str| Configuration::read(&dir.join(package)).unwrap();
        assert_eq!(read("own").timeout_ceiling, minutes(5));
        assert_eq!(read("kept").timeout_ceiling, minutes(30));
    }

    /// A directory of its own for one test, with `files` written in it.
    fn written(label: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mutastic-config-{label}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        for (path, text) in files {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        dir
    }

    /// `exclude-tests:` leaves tests out of every run, each with the reason the summary shows.
    #[test]
    fn a_test_is_left_out_with_its_reason() {
        let config = parse(
            "exclude-tests:\n  - test: checkout::tests::pays_once\n    reason: shares port 8080 # #412\n\
             \x20 - test: 'tests'\n    reason: \"a race\"\n",
        )
        .unwrap();
        assert_eq!(
            config.exclude_tests,
            [
                ExcludedTest {
                    test: "checkout::tests::pays_once".into(),
                    reason: "shares port 8080".into(),
                },
                ExcludedTest {
                    test: "tests".into(),
                    reason: "a race".into(),
                },
            ]
        );
    }

    #[test]
    fn a_file_adds_what_it_extends_after_its_own() {
        let top =
            "rules:\n  - skip: shop::**\n    reason: top\n  - skip: a::**\n    reason: both\n\
                   plugins:\n  - rules/top.yaml\noperators:\n  - stronger\n\
                   exclude-tests:\n  - test: tests::races\n    reason: top's\n  - test: tests::ports\n    reason: top\n";
        let dir = written(
            "extends",
            &[
                ("mutastic.yaml", top),
                (
                    "shop/mutastic.yaml",
                    "extends: ../mutastic.yaml\nrules:\n  - skip: shop::legacy::**\n    reason: own\n\
                     \x20 - skip: a::**\n    reason: both\nplugins:\n  - rules/shop.yaml\n  - ../rules/top.yaml\n\
                     exclude-tests:\n  - test: tests::races\n    reason: shop's\n",
                ),
                (
                    "billing/mutastic.yaml",
                    "extends: ../mutastic.yaml\noperators:\n  - defaults\n",
                ),
            ],
        );
        let shop = Configuration::read(&dir.join("shop")).unwrap();
        let reasons: Vec<&str> = shop.rules.iter().map(|r| r.reason.as_str()).collect();
        assert_eq!(reasons, ["own", "both", "top"]);
        assert_eq!(
            shop.plugins,
            [dir.join("shop/rules/shop.yaml"), dir.join("rules/top.yaml")]
        );
        assert_eq!(shop.operators, ["stronger"]);
        let left_out: Vec<(&str, &str)> = shop
            .exclude_tests
            .iter()
            .map(|test| (test.test.as_str(), test.reason.as_str()))
            .collect();
        assert_eq!(
            left_out,
            [("tests::races", "shop's"), ("tests::ports", "top")],
            "the file's own reason is the one kept"
        );
        let billing = Configuration::read(&dir.join("billing")).unwrap();
        assert_eq!(billing.operators, ["defaults"]);
        assert_eq!(billing.rules.len(), 2);
        // Only what a file names applies: a package's file without extends: is all there is.
        std::fs::write(
            dir.join("billing/mutastic.yaml"),
            "operators:\n  - defaults\n",
        )
        .unwrap();
        assert!(Configuration::read(&dir.join("billing"))
            .unwrap()
            .rules
            .is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extends_names_a_file_that_is_there_and_never_comes_round() {
        let dir = written(
            "extends-errors",
            &[
                ("missing/mutastic.yaml", "extends: ../top.yaml\n"),
                ("a/mutastic.yaml", "extends: ../mutastic.yaml\n"),
                ("mutastic.yaml", "extends: a/mutastic.yaml\n"),
                ("deep/mutastic.yaml", "extends: ../bad.yaml\n"),
                ("bad.yaml", "rules:\n  - skip: x::**\n"),
            ],
        );
        let error = |sub: &str| Configuration::read(&dir.join(sub)).unwrap_err().message;
        assert_eq!(
            error("missing"),
            "mutastic.yaml line 1: extends ../top.yaml, which isn't there"
        );
        assert_eq!(
            error("a"),
            "extends: goes round in a loop: mutastic.yaml extends ../mutastic.yaml extends mutastic.yaml"
        );
        // An error in the file extended names that file, from the first.
        assert_eq!(
            error("deep"),
            "../bad.yaml line 2: the rule `skip: x::**` needs a `reason:` under it"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_nearest_file_is_looked_for_up_to_the_repository_or_the_run() {
        let dir = written(
            "nearest",
            &[("mutastic.yaml", "rules:\n"), ("shop/src/lib.rs", "")],
        );
        let dir = std::fs::canonicalize(dir).unwrap();
        let shop = dir.join("shop");
        // Outside a repository, no higher than the run's directory.
        assert_eq!(nearest(&shop, &dir), Some(dir.clone()));
        assert_eq!(nearest(&shop, &shop), None);
        // In one, up to its top, however it's run.
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        assert_eq!(nearest(&shop, &shop), Some(dir.clone()));
        // The package's own file is nearer.
        std::fs::write(shop.join("mutastic.yaml"), "rules:\n").unwrap();
        assert_eq!(nearest(&shop, &dir), Some(shop.clone()));
        // Never above the repository's top.
        std::fs::remove_file(shop.join("mutastic.yaml")).unwrap();
        std::fs::create_dir_all(shop.join(".git")).unwrap();
        assert_eq!(nearest(&shop, &shop), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn paths_are_named_from_the_first_files_directory() {
        assert_eq!(
            relative(Path::new("/a/b"), Path::new("/a/mutastic.yaml")),
            Path::new("../mutastic.yaml")
        );
        assert_eq!(relative(Path::new("/a"), Path::new("/a")), Path::new(""));
        assert_eq!(normalized(Path::new("./a/b/../c")), Path::new("a/c"));
        assert_eq!(normalized(Path::new("../a")), Path::new("../a"));
    }

    #[test]
    fn patterns_match_paths_as_the_java_editions_match_classes() {
        let rule = |target: &str| Rule {
            target: target.into(),
            reason: "r".into(),
            ..Rule::default()
        };
        assert!(rule("shop::generated::**").covers("shop::generated::proto::Order::new"));
        assert!(rule("shop::*::Money").covers("shop::money::Money::add"));
        assert!(rule("shop::*").covers("shop::total"));
        assert!(!rule("shop::*").covers("shop::money::Money::add"));
        assert!(rule("shop::Money::fmt").covers("shop::Money::fmt"));
        assert!(!rule("shop::Money::fmt").covers("shop::Money::add"));
        assert!(!rule("shop::*::Money").covers("shop::a::b::Money::add"));
    }

    #[test]
    fn operators_are_chosen_as_the_java_edition_chooses_them() {
        let chosen = resolve(&["defaults".into(), "-mutastic/boundary".into()], "t").unwrap();
        assert!(!chosen.contains(&crate::sites::BOUNDARY));
        assert!(chosen.contains(&crate::sites::MATH));
        let stream = resolve(&["stream".into()], "t").unwrap();
        assert!(!stream.is_empty() && stream.iter().all(|id| id.starts_with("stream/")));
        let refused = resolve(&["nope".into()], "--operators")
            .unwrap_err()
            .message;
        assert!(
            refused.starts_with("--operators: nope isn't an operator mutastic has.\n"),
            "{refused}"
        );
        assert!(
            refused.contains("a set (defaults, stronger, extended or all), a namespace ("),
            "{refused}"
        );
        assert!(
            refused.ends_with("\n  mutastic mutants --operators=all"),
            "{refused}"
        );
        assert!(resolve(&["strong".into()], "--operators")
            .unwrap_err()
            .message
            .starts_with(
            "--operators: strong isn't an operator mutastic has. There's one called stronger.\n"
        ));
    }

    #[test]
    fn the_gate_chooses_its_own_operators() {
        let config =
            parse("operators:\n  - defaults\ngate:\n  operators:\n    - stronger\n    - -stream\n")
                .unwrap();
        assert_eq!(config.operators, ["defaults"]);
        assert_eq!(config.gate_operators, ["stronger", "-stream"]);
        assert_eq!(config.gate_operators_from[1], "mutastic.yaml line 6");
        for refused in [
            "gate: {operators: stronger}\n",
            "gate:\n  - stronger\n",
            "gate:\n  threshold:\n",
        ] {
            let error = parse(refused).unwrap_err();
            assert!(
                error.contains("gate: takes operators:, a list under it"),
                "{refused}: {error}"
            );
        }
    }

    #[test]
    fn an_operator_written_in_the_file_is_named_by_its_line() {
        let config = parse("operators:\n  - defaults\n  - mutastic/bondary\n").unwrap();
        let refused = resolve_from(&config.operators, &config.operators_from)
            .unwrap_err()
            .message;
        assert!(
            refused.starts_with(
                "mutastic.yaml line 3: mutastic/bondary isn't an operator mutastic has. There's \
                 one called mutastic/boundary.\n"
            ),
            "{refused}"
        );
    }

    #[test]
    fn a_plugin_is_named_by_its_line() {
        let config = parse("plugins:\n  - rules/a.yaml\n  - rules/b.yaml\n").unwrap();
        assert_eq!(
            config.plugins_from,
            ["mutastic.yaml line 2", "mutastic.yaml line 3"]
        );
        let refused = crate::callrules::read(&config.plugins[1], &config.plugins_from[1]);
        assert_eq!(
            refused.unwrap_err().message,
            "mutastic.yaml line 3: the plugin rules/b.yaml isn't there. A plugin's path is from \
             the directory of the file that names it."
        );
    }
}
