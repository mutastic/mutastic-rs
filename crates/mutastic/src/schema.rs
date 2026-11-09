//! The mutant schema: every mutant written into one copy of the source, each behind its own
//! switch, so one build holds them all and an environment variable picks the live one.
//!
//! A site's guard reads `(match crate::__mutastic::at(7) { 7 => { (a) <= (b) } _ => { a < b } })`.
//! A statement's guard has no parentheses, and is a statement itself:
//! `match crate::__mutastic::at(9) { 9 => {} _ => { v.push(x); } }`. A mutant that adds code (a
//! parameter set at the start of a function, a guard on a match arm) is written where it goes, with
//! its own read of the switch. A function's whole body is guarded inside a block of its own:
//! `fn f() -> u32 { match crate::__mutastic::at(4) { 4 => { 0 } _ => { { .. } } } }`.
//! Mutants nested inside a site go only in its last arm, as the original code: one mutant is live per
//! process, so a mutated arm can hold plain text. That keeps the rewrite as long as the source,
//! not exponential in nesting, and means a compile error inside a mutated arm names its mutant.
//! Copies never add a line (whitespace in them is collapsed), and the runtime goes at the end of
//! the crate root, so line numbers in panics and backtraces are the user's.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::Path;

use crate::sites::{Crate, Form, Function, Mutant};

/// Where things ended up in a rewritten file, for blaming compile errors.
#[derive(Debug, Default)]
pub struct Emitted {
    pub text: String,
    /// Each guard's whole text, with the mutants it holds.
    pub guards: Vec<(Range<usize>, Vec<usize>)>,
    /// Each mutant's arm: text that exists only because the mutant was written.
    pub arms: Vec<(Range<usize>, usize)>,
    /// Each function's text, with its index.
    pub functions: Vec<(Range<usize>, usize)>,
    /// Each probe's text, with its function's index.
    pub probes: Vec<(Range<usize>, usize)>,
}

struct Site {
    range: Range<usize>,
    /// Indices into `Crate::mutants`.
    mutants: Vec<usize>,
    children: Vec<Site>,
    /// How the guard is written: in parentheses, as a statement, or as a function's body.
    form: Form,
}

/// Text written at one place in the original: an inputs recorder, or a mutant's added code.
#[derive(Clone)]
struct Insert {
    offset: usize,
    text: String,
    /// The mutant whose code it is.
    mutant: Option<usize>,
    /// The function whose probe it is: written after the function, never inside a guard.
    probe: Option<usize>,
}

/// The rewritten text of one file, with only the `live` mutants (indices) written in, an inputs
/// recorder at the top of each function in `recorders`, and a probe after each in `probes`.
pub fn emit(
    found: &Crate,
    file: &Path,
    text: &str,
    live: &BTreeSet<usize>,
    recorders: &BTreeSet<usize>,
    probes: &BTreeSet<usize>,
) -> Emitted {
    let sites = nest(found, file, live);
    let mut inserts: Vec<Insert> = found
        .functions
        .iter()
        .enumerate()
        .filter(|(index, function)| function.file == file && recorders.contains(index))
        .filter(|(_, function)| !function.numbers.is_empty())
        .map(|(_, function)| Insert {
            offset: function.body_open,
            text: recorder(function),
            mutant: None,
            probe: None,
        })
        .collect();
    for (index, function) in found.functions.iter().enumerate() {
        if let (true, true, Some(callable)) = (
            function.file == file,
            probes.contains(&index),
            &function.callable,
        ) {
            inserts.push(Insert {
                offset: callable.after,
                text: probe(function.ordinal, callable),
                mutant: None,
                probe: Some(index),
            });
        }
    }
    for (index, mutant) in found.mutants.iter().enumerate() {
        if let Form::Insert { text } = &mutant.form {
            if mutant.file == file && live.contains(&index) {
                inserts.push(Insert {
                    offset: mutant.site.start,
                    text: text.replace("{n}", &mutant.ordinal.to_string()),
                    mutant: Some(index),
                    probe: None,
                });
            }
        }
    }
    // The recorder first, so it sees the values the function was called with.
    inserts.sort_by_key(|insert| {
        (
            insert.offset,
            insert.mutant.is_some(),
            insert.mutant,
            insert.probe,
        )
    });
    let mut out = Emitter {
        text,
        found,
        inserts,
        next_insert: 0,
        depth: 0,
        emitted: Emitted::default(),
        offsets: Vec::new(),
    };
    out.range(0, text.len(), &sites);
    let functions: Vec<(Range<usize>, usize)> = found
        .functions
        .iter()
        .enumerate()
        .filter(|(_, function)| function.file == file)
        .map(|(index, function)| {
            (
                out.map(function.range.start)..out.map(function.range.end),
                index,
            )
        })
        .collect();
    out.emitted.functions = functions;
    out.emitted
}

/// `crate::__mutastic::seen(3, &[("total", total as i128)]);`
fn recorder(function: &Function) -> String {
    let values: Vec<String> = function
        .numbers
        .iter()
        .map(|(name, _)| format!("(\"{name}\", {name} as i128)"))
        .collect();
    format!(
        " crate::__mutastic::seen({}, &[{}]);",
        function.ordinal,
        values.join(", ")
    )
}

/// A test that calls the function on each line of whole numbers mutastic gives it, and writes down
/// what each call returned: how a suggested test's values are found, by running the code, with
/// and without a mutant. Written after the function, or its `impl`, on the same line.
fn probe(ordinal: u32, callable: &crate::sites::callable::Callable) -> String {
    let types = callable.types();
    let lets: String = types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            format!(
                "let __a{index} = <{ty} as ::core::convert::TryFrom<i128>>::try_from(*__v.get({index})?).ok()?; "
            )
        })
        .collect();
    let names: Vec<String> = (0..types.len())
        .map(|index| format!("__a{index}"))
        .collect();
    let call = callable.call(&names, "");
    let cfgs: String = callable.cfgs.iter().map(|cfg| format!("{cfg} ")).collect();
    format!(
        " {cfgs}#[::core::prelude::v1::test] #[doc(hidden)] #[allow(warnings, clippy::all)] fn {}{ordinal}() {{ \
         crate::__mutastic::probe(&|__v: &[i128]| -> ::core::option::Option<crate::__mutastic::Shown> {{ \
         {lets}::core::option::Option::Some(crate::__mutastic::shown(&{call})) }}); }}",
        crate::sites::callable::PROBE
    )
}

/// The live mutants' sites in `file`, nested by containment.
fn nest(found: &Crate, file: &Path, live: &BTreeSet<usize>) -> Vec<Site> {
    let mut flat: Vec<Site> = Vec::new();
    for (index, mutant) in found.mutants.iter().enumerate() {
        if mutant.file != file || !live.contains(&index) {
            continue;
        }
        if matches!(mutant.form, Form::Insert { .. }) {
            continue;
        }
        match flat.iter_mut().find(|site| site.range == mutant.site) {
            Some(site) => site.mutants.push(index),
            None => flat.push(Site {
                range: mutant.site.clone(),
                mutants: vec![index],
                children: Vec::new(),
                form: mutant.form.clone(),
            }),
        }
    }
    flat.sort_by(|a, b| (a.range.start, b.range.end).cmp(&(b.range.start, a.range.end)));
    let mut roots: Vec<Site> = Vec::new();
    for site in flat {
        insert(&mut roots, site);
    }
    roots
}

fn insert(siblings: &mut Vec<Site>, site: Site) {
    if let Some(last) = siblings.last_mut() {
        if last.range.start <= site.range.start && site.range.end <= last.range.end {
            return insert(&mut last.children, site);
        }
    }
    siblings.push(site);
}

struct Emitter<'a> {
    text: &'a str,
    found: &'a Crate,
    inserts: Vec<Insert>,
    next_insert: usize,
    /// How many guards deep the text being written is: a probe waits until it's out of them all.
    depth: usize,
    emitted: Emitted,
    /// (original offset, output offset) at the start of each verbatim copy.
    offsets: Vec<(usize, usize)>,
}

impl Emitter<'_> {
    fn verbatim(&mut self, from: usize, to: usize) {
        let mut at = from;
        while let Some(insert) = self.inserts.get(self.next_insert).cloned() {
            if insert.offset < at
                || insert.offset > to
                || (insert.probe.is_some() && self.depth > 0)
            {
                break;
            }
            self.copy(at, insert.offset);
            let start = self.emitted.text.len();
            self.emitted.text.push_str(&insert.text);
            if let Some(mutant) = insert.mutant {
                let range = start..self.emitted.text.len();
                self.emitted.arms.push((range.clone(), mutant));
                self.emitted.guards.push((range, vec![mutant]));
            }
            if let Some(function) = insert.probe {
                let range = start..self.emitted.text.len();
                self.emitted.probes.push((range, function));
            }
            self.next_insert += 1;
            at = insert.offset;
        }
        self.copy(at, to);
    }

    fn copy(&mut self, from: usize, to: usize) {
        self.offsets.push((from, self.emitted.text.len()));
        self.emitted.text.push_str(&self.text[from..to]);
    }

    fn range(&mut self, from: usize, to: usize, sites: &[Site]) {
        let mut at = from;
        for site in sites {
            self.verbatim(at, site.range.start);
            self.site(site);
            at = site.range.end;
        }
        self.verbatim(at, to);
    }

    /// `(match crate::__mutastic::at(7) { 7 => { (a) <= (b) } 8 => { true } _ => { a < b } })`:
    /// one read of the switch per site, however many mutants it holds. The number given to `at`
    /// is the site's first mutant, which the coverage pass records when the site is reached.
    fn site(&mut self, site: &Site) {
        let start = self.emitted.text.len();
        let first = site
            .mutants
            .iter()
            .map(|&index| self.found.mutants[index].ordinal)
            .min()
            .unwrap_or(0);
        let (open, close) = match site.form {
            Form::Statement => ("", " } }"),
            // A function's body must stay a block.
            Form::Body => ("{ ", " } } }"),
            _ => ("(", " } })"),
        };
        self.emitted
            .text
            .push_str(&format!("{open}match crate::__mutastic::at({first}) {{ "));
        for &index in &site.mutants {
            let mutant: &Mutant = &self.found.mutants[index];
            self.emitted
                .text
                .push_str(&format!("{} => {{ ", mutant.ordinal));
            let arm = self.emitted.text.len();
            self.emitted.text.push_str(&mutant.replacement);
            self.emitted
                .arms
                .push((arm..self.emitted.text.len(), index));
            self.emitted.text.push_str(" } ");
        }
        self.emitted.text.push_str("_ => { ");
        self.depth += 1;
        self.range(site.range.start, site.range.end, &site.children);
        self.depth -= 1;
        self.emitted.text.push_str(close);
        self.emitted
            .guards
            .push((start..self.emitted.text.len(), site.mutants.clone()));
    }

    /// Where an original offset that was copied verbatim ended up.
    fn map(&self, offset: usize) -> usize {
        let at = self.offsets.partition_point(|(from, _)| *from <= offset);
        match at.checked_sub(1).and_then(|i| self.offsets.get(i)) {
            Some((from, out)) => out + (offset - from),
            None => offset,
        }
    }
}

/// The module that holds the switches, appended to each crate root. It has no dependencies, so
/// `Cargo.toml` is never touched and `--locked`, `--offline` and vendored builds still work.
///
/// The switch is read once, before `main`, where the platform runs initialisers (Linux, the BSDs,
/// macOS): reading the environment allocates, and done there it can never happen inside code that
/// must not allocate or take a lock. Elsewhere, or if the linker drops the initialiser, the first
/// read does it, as `at` still checks.
pub fn runtime(sites: usize, edition: &str) -> String {
    // Edition 2024 wants `link_section` marked unsafe; earlier editions, and older compilers,
    // don't take the marked form.
    let section = |name: &str| {
        if edition >= "2024" {
            format!("unsafe(link_section = \"{name}\")")
        } else {
            format!("link_section = \"{name}\"")
        }
    };
    let apple = section("__DATA,__mod_init_func");
    let elf = section(".init_array");
    format!(
        r#"

#[doc(hidden)]
#[allow(warnings, clippy::all)]
pub(crate) mod __mutastic {{
    // Written by mutastic into its own copy of this crate: your source has none of this.
    extern crate std;
    use std::sync::atomic::{{AtomicBool, AtomicU32, Ordering::Relaxed}};
    const UNREAD: u32 = u32::MAX;
    const COVERING: u32 = u32::MAX - 1;
    const SITES: usize = {sites};
    static ACTIVE: AtomicU32 = AtomicU32::new(UNREAD);
    static HIT: [AtomicBool; SITES + 1] = [const {{ AtomicBool::new(false) }}; SITES + 1];
    static OUT: std::sync::OnceLock<std::sync::Mutex<Option<std::fs::File>>> = std::sync::OnceLock::new();
    static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<std::string::String>>> = std::sync::OnceLock::new();

    /// The live mutant's number, 0 for none; in the coverage pass, records that the site whose
    /// first mutant is `site` was reached, and gives 0. Inlined even in a debug build, since it
    /// runs at every site the tests reach.
    #[inline(always)]
    pub(crate) fn at(site: u32) -> u32 {{
        let mut active = ACTIVE.load(Relaxed);
        if active == UNREAD {{
            active = read();
        }}
        if active == COVERING {{
            hit(site);
            return 0;
        }}
        active
    }}

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd", target_os = "macos", target_os = "ios"))]
    #[used]
    #[cfg_attr(any(target_os = "macos", target_os = "ios"), {apple})]
    #[cfg_attr(not(any(target_os = "macos", target_os = "ios")), {elf})]
    static EAGER: extern "C" fn() = {{
        extern "C" fn eager() {{
            read();
        }}
        eager
    }};

    #[cold]
    fn read() -> u32 {{
        let active = if std::env::var_os("MUTASTIC_COVERAGE").is_some() {{
            COVERING
        }} else {{
            std::env::var("MUTASTIC_ACTIVE").ok().and_then(|v| v.parse().ok()).unwrap_or(0)
        }};
        ACTIVE.store(active, Relaxed);
        active
    }}

    fn write(line: &str) {{
        let out = OUT.get_or_init(|| {{
            std::sync::Mutex::new(std::env::var_os("MUTASTIC_COVERAGE").and_then(|path| {{
                std::fs::OpenOptions::new().create(true).append(true).open(path).ok()
            }}))
        }});
        if let Ok(mut file) = out.lock() {{
            if let Some(file) = file.as_mut() {{
                let _ = std::io::Write::write_all(file, line.as_bytes());
            }}
        }}
    }}

    #[cold]
    fn hit(id: u32) {{
        if let Some(flag) = HIT.get(id as usize) {{
            if !flag.swap(true, Relaxed) {{
                write(&std::format!("h {{id}}\n"));
            }}
        }}
    }}

    pub(crate) type Shown = std::string::String;

    /// What a probe's call returned, as `{{:?}}` writes it: a literal, for what a probe calls.
    pub(crate) fn shown<T: core::fmt::Debug + ?Sized>(value: &T) -> Shown {{
        std::format!("{{value:?}}")
    }}

    /// A probe's run: `call` on each line of whole numbers in the file `MUTASTIC_PROBE` names,
    /// writing a line for each to the file `MUTASTIC_PROBE_OUT` names as it goes: `r` and what it
    /// returned, `p` when it panicked, `s` when the numbers don't fit its types, and, with
    /// `MUTASTIC_PROBE_TWICE`, `x` when a second call returned something else.
    pub(crate) fn probe(call: &dyn Fn(&[i128]) -> Option<Shown>) {{
        let (Some(input), Some(output)) = (std::env::var_os("MUTASTIC_PROBE"), std::env::var_os("MUTASTIC_PROBE_OUT")) else {{
            return;
        }};
        let twice = std::env::var_os("MUTASTIC_PROBE_TWICE").is_some();
        std::panic::set_hook(std::boxed::Box::new(|_| {{}}));
        let text = std::fs::read_to_string(input).unwrap_or_default();
        let Ok(mut out) = std::fs::File::create(output) else {{
            return;
        }};
        for line in text.lines() {{
            let values: std::vec::Vec<i128> = line.split(' ').filter_map(|word| word.parse().ok()).collect();
            let once = || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(&values)));
            let written = match once() {{
                Ok(None) => std::string::String::from("s"),
                Ok(Some(value)) => {{
                    if twice && once().ok().flatten().as_ref() != Some(&value) {{
                        std::string::String::from("x")
                    }} else {{
                        std::format!("r {{value}}")
                    }}
                }}
                Err(_) => std::string::String::from("p"),
            }};
            let _ = std::io::Write::write_all(&mut out, std::format!("{{written}}\n").as_bytes());
        }}
    }}

    /// Records the whole numbers a function was called with, in the coverage pass only.
    pub(crate) fn seen(function: u32, values: &[(&str, i128)]) {{
        let mut active = ACTIVE.load(Relaxed);
        if active == UNREAD {{
            active = read();
        }}
        if active != COVERING {{
            return;
        }}
        let mut line = std::format!("s {{function}}");
        for (name, value) in values {{
            line.push_str(&std::format!(" {{name}}={{value}}"));
        }}
        line.push('\n');
        let seen = SEEN.get_or_init(Default::default);
        if let Ok(mut seen) = seen.lock() {{
            if seen.len() < 4096 && seen.insert(line.clone()) {{
                write(&line);
            }}
        }}
    }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites;
    use std::path::PathBuf;

    static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn crate_of(source: &str) -> Crate {
        let dir = std::env::temp_dir().join(format!(
            "mutastic-schema-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let found = sites::find(&dir, "s", &[PathBuf::from("src/lib.rs")], sites::DEFAULTS);
        std::fs::remove_dir_all(&dir).ok();
        found
    }

    #[test]
    fn nested_sites_go_only_in_the_original_arm() {
        let source = "pub fn f(a: u32, b: u32) {\n    if a + 1 < b { g() }\n}\n";
        let found = crate_of(source);
        let live: BTreeSet<usize> = (0..found.mutants.len()).collect();
        let emitted = emit(
            &found,
            Path::new("src/lib.rs"),
            source,
            &live,
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        assert_eq!(
            emitted.text,
            "pub fn f(a: u32, b: u32) {\n    if (match crate::__mutastic::at(1) { 1 => { (a + 1) <= (b) } \
             2 => { true } 3 => { false } \
             _ => { (match crate::__mutastic::at(4) { 4 => { (a) - (1) } _ => { a + 1 } }) < b } }) { g() }\n}\n"
        );
        assert_eq!(emitted.text.lines().count(), source.lines().count());
        let (arm, mutant) = &emitted.arms[0];
        assert_eq!(&emitted.text[arm.clone()], "(a + 1) <= (b)");
        assert_eq!(*mutant, 0);
    }

    #[test]
    fn withdrawn_mutants_leave_the_code_as_it_was() {
        let source = "pub fn f(a: u32) -> u32 { a + 1 }";
        let found = crate_of(source);
        let emitted = emit(
            &found,
            Path::new("src/lib.rs"),
            source,
            &BTreeSet::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
        );
        assert_eq!(emitted.text, source);
    }

    #[test]
    fn a_recorder_goes_at_the_top_of_the_body_on_the_same_line() {
        let source = "pub fn f(a: u32, s: &str) -> u32 {\n    a + 1\n}";
        let found = crate_of(source);
        let emitted = emit(
            &found,
            Path::new("src/lib.rs"),
            source,
            &BTreeSet::new(),
            &BTreeSet::from([0]),
            &BTreeSet::new(),
        );
        assert_eq!(
            emitted.text,
            "pub fn f(a: u32, s: &str) -> u32 { crate::__mutastic::seen(1, &[(\"a\", a as i128)]);\n    a + 1\n}"
        );
        assert_eq!(emitted.functions[0].0, 0..emitted.text.len());
    }
}
