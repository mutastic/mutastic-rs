//! The mutant schema: every mutant written into one copy of the source, each behind its own
//! switch, so one build holds them all and an environment variable picks the live one.
//!
//! A site's guard reads `(if crate::__mutastic::on(7) { (a) <= (b) } else { a < b })`. Mutants
//! nested inside a site go only in its `else` arm, as the original code: one mutant is live per
//! process, so a mutated arm can hold plain text. That keeps the rewrite as long as the source,
//! not exponential in nesting, and means a compile error inside a mutated arm names its mutant.
//! Copies never add a line (whitespace in them is collapsed), and the runtime goes at the end of
//! the crate root, so line numbers in panics and backtraces are the user's.

use std::collections::BTreeSet;
use std::ops::Range;
use std::path::Path;

use crate::sites::{Crate, Function, Mutant};

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
}

struct Site {
    range: Range<usize>,
    /// Indices into `Crate::mutants`.
    mutants: Vec<usize>,
    children: Vec<Site>,
}

/// The rewritten text of one file, with only the `live` mutants (indices) written in, and an
/// inputs recorder at the top of each listed function.
pub fn emit(
    found: &Crate,
    file: &Path,
    text: &str,
    live: &BTreeSet<usize>,
    recorders: &BTreeSet<usize>,
) -> Emitted {
    let sites = nest(found, file, live);
    let mut inserts: Vec<(usize, String)> = found
        .functions
        .iter()
        .enumerate()
        .filter(|(index, function)| function.file == file && recorders.contains(index))
        .filter(|(_, function)| !function.numbers.is_empty())
        .map(|(_, function)| (function.body_open, recorder(function)))
        .collect();
    inserts.sort();
    let mut out = Emitter {
        text,
        found,
        inserts,
        next_insert: 0,
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

/// The live mutants' sites in `file`, nested by containment.
fn nest(found: &Crate, file: &Path, live: &BTreeSet<usize>) -> Vec<Site> {
    let mut flat: Vec<Site> = Vec::new();
    for (index, mutant) in found.mutants.iter().enumerate() {
        if mutant.file != file || !live.contains(&index) {
            continue;
        }
        match flat.iter_mut().find(|site| site.range == mutant.site) {
            Some(site) => site.mutants.push(index),
            None => flat.push(Site {
                range: mutant.site.clone(),
                mutants: vec![index],
                children: Vec::new(),
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
    inserts: Vec<(usize, String)>,
    next_insert: usize,
    emitted: Emitted,
    /// (original offset, output offset) at the start of each verbatim copy.
    offsets: Vec<(usize, usize)>,
}

impl Emitter<'_> {
    fn verbatim(&mut self, from: usize, to: usize) {
        let mut at = from;
        while let Some((offset, insert)) = self.inserts.get(self.next_insert).cloned() {
            if offset < at || offset > to {
                break;
            }
            self.copy(at, offset);
            self.emitted.text.push_str(&insert);
            self.next_insert += 1;
            at = offset;
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

    fn site(&mut self, site: &Site) {
        let start = self.emitted.text.len();
        self.emitted.text.push('(');
        for &index in &site.mutants {
            let mutant: &Mutant = &self.found.mutants[index];
            self.emitted
                .text
                .push_str(&format!("if crate::__mutastic::on({}) {{ ", mutant.ordinal));
            let arm = self.emitted.text.len();
            self.emitted.text.push_str(&mutant.replacement);
            self.emitted
                .arms
                .push((arm..self.emitted.text.len(), index));
            self.emitted.text.push_str(" } else ");
        }
        self.emitted.text.push_str("{ ");
        self.range(site.range.start, site.range.end, &site.children);
        self.emitted.text.push_str(" })");
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
pub fn runtime(sites: usize) -> String {
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

    /// Whether mutant `id` is the live one.
    #[inline]
    pub(crate) fn on(id: u32) -> bool {{
        let mut active = ACTIVE.load(Relaxed);
        if active == UNREAD {{
            active = read();
        }}
        if active == COVERING {{
            hit(id);
            return false;
        }}
        active == id
    }}

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
            if seen.len() < 256 && seen.insert(line.clone()) {{
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
        );
        assert_eq!(
            emitted.text,
            "pub fn f(a: u32, b: u32) {\n    if (if crate::__mutastic::on(1) { (a + 1) <= (b) } \
             else if crate::__mutastic::on(2) { true } else if crate::__mutastic::on(3) { false } \
             else { (if crate::__mutastic::on(4) { (a) - (1) } else { a + 1 }) < b }) { g() }\n}\n"
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
        );
        assert_eq!(
            emitted.text,
            "pub fn f(a: u32, s: &str) -> u32 { crate::__mutastic::seen(1, &[(\"a\", a as i128)]);\n    a + 1\n}"
        );
        assert_eq!(emitted.functions[0].0, 0..emitted.text.len());
    }
}
