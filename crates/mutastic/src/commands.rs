//! The commands and options `mutastic` takes, declared once. The parser accepts an option only as
//! declared here, and only on the commands it names; `--help` is written from these ([`crate::help`]);
//! and so is `docs/commands.json`, the docs site's reference pages, in the Java edition's shape
//! (`mutastic.commands/v1`). The docs test fails while the committed copy differs from
//! [`model`]: `MUTASTIC_DOCS_WRITE=1 cargo test --test docs` writes it again.

use serde_json::{json, Value};

use crate::failure::{Code, Failure};

/// The version of `commands.json`'s shape, the Java edition's, for the docs site to check.
pub const SCHEMA: &str = "mutastic.commands/v1";

/// Where the Rust edition's docs are served.
pub const DOCS: &str = "https://docs.mutastic.io/rust/";

/// A mutant of the pricing fixture, for the examples: its boundary at `total == 100`.
const ID: &str = "mb2f8ae57";

pub struct Command {
    /// As it's typed after `mutastic`: `run`, `gate open`.
    pub name: &'static str,
    /// Its page in the docs: `run`, `gate-open`.
    pub page: &'static str,
    /// The words it takes, as an error about them shows them.
    pub synopsis: &'static str,
    pub summary: &'static str,
    pub detail: &'static str,
    pub exits: &'static str,
    pub takes_mutant_id: bool,
    pub examples: &'static [&'static str],
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `--propose`: nothing after it.
    Flag,
    /// `--threshold=70`.
    Value,
    /// `--since`, or `--since=main`.
    OptionalValue,
    /// `--report=json --report=html`: given again, it adds.
    Repeated,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Flag => "flag",
            Kind::Value => "value",
            Kind::OptionalValue => "optional_value",
            Kind::Repeated => "repeated",
        }
    }
}

pub struct CliOption {
    /// Without its dashes: `threshold`.
    pub name: &'static str,
    /// What its value stands for, `N`; empty for a flag.
    pub value: &'static str,
    pub kind: Kind,
    /// The heading it's listed under, as the Java edition groups its options.
    pub group: &'static str,
    /// Every value it takes, when it takes only a few.
    pub choices: &'static [&'static str],
    pub help: &'static str,
    /// The commands that take it.
    pub commands: &'static [&'static str],
    /// The environment variables that stand in for it.
    pub environment: &'static str,
    /// What an example adds to a command for it, `--threshold=70`; or, starting `mutastic `, the
    /// whole example, for an option only one command takes.
    pub example: &'static str,
}

impl CliOption {
    /// `--threshold=N`, `--since[=REF]`, `--propose`.
    pub fn synopsis(&self) -> String {
        match self.kind {
            Kind::Flag => format!("--{}", self.name),
            Kind::OptionalValue => format!("--{}[={}]", self.name, self.value),
            Kind::Value | Kind::Repeated => format!("--{}={}", self.name, self.value),
        }
    }

    /// The option at work on `command`, as the command line writes it.
    pub fn example_for(&self, command: &Command) -> String {
        if self.example.starts_with("mutastic ") {
            return self.example.to_string();
        }
        // `ignore --prune` names no mutant: it takes away every ignore that has lapsed.
        let id = if command.takes_mutant_id && self.name != "prune" {
            format!(" {ID}")
        } else {
            String::new()
        };
        format!("mutastic {}{id} {}", command.name, self.example)
    }
}

/// A variable of the environment mutastic reads, or a status it exits with.
pub struct Entry {
    pub name: &'static str,
    pub meaning: &'static str,
    pub detail: &'static str,
}

const EVERY_COMMAND: &[&str] = &[
    "run",
    "verify",
    "explain",
    "ignore",
    "gate open",
    "gate check",
    "gate close",
    "mutants",
    "next",
    "baseline",
];

pub const COMMANDS: &[Command] = &[
    Command {
        name: "run",
        page: "run",
        synopsis: "mutastic run [DIR] [OPTIONS]",
        summary: "Mutate the package and run its tests against each mutant",
        detail: "DIR is the package's directory, here unless given; in a workspace, a member's. The \
                 run passes when the score reaches the threshold, or with --fail-on-new when every \
                 mutant no test catches is in the baseline. A full run, one without --only, \
                 writes report files, and offers the commands to work on each missed mutant. Its \
                 progress goes to stderr, so stdout holds only the result. --shard runs one share \
                 of the mutants, for splitting a run across CI jobs, and --from-shards puts the \
                 shares back together as the whole run.",
        exits: "0 when the score reaches the threshold, 1 when it doesn't; with --fail-on-new, 0 \
                when every mutant no test catches is in the baseline, 1 when one isn't.",
        takes_mutant_id: false,
        examples: &[
            "mutastic run",
            "mutastic run fixtures/pricing --threshold=70",
            "mutastic run --since=main",
            "mutastic run --fail-on-new",
            "mutastic run fixtures/pricing --operators=stronger",
            "mutastic run --shard=2/4 --threshold=0",
            "mutastic run --from-shards=shares --threshold=70",
        ],
    },
    Command {
        name: "verify",
        page: "verify",
        synopsis: "mutastic verify ID... [DIR]",
        summary: "Run mutants' tests: it passes when they catch every one not skipped",
        detail: "How a fix is proved: each missed mutant offers it as \"done when\", and a card \
                 with several mutants names them all, split by commas; spaces split them too, and \
                 the directory comes after them. An id names its mutant whatever options showed \
                 it, so verify needs no --operators or --unsafe to find one.",
        exits: "0 when it catches each mutant named, or the mutant is skipped or crashed where a \
                crash proves nothing; 1 while one is still missed or not covered.",
        takes_mutant_id: true,
        examples: &[
            "mutastic verify mb2f8ae57",
            "mutastic verify mb2f8ae57 fixtures/pricing",
            "mutastic verify mb2f8ae57 m95f96847 fixtures/pricing",
        ],
    },
    Command {
        name: "explain",
        page: "explain",
        synopsis: "mutastic explain ID [DIR]",
        summary: "Everything known of one mutant, with its tests run",
        detail: "Its card when it's missed, or what became of it otherwise, then what the change \
                 is, its operator, how its id is made, where it is, and how the last run of every \
                 mutant, by run or next, went for it, from that run's mutation.json.",
        exits: "0 whatever the mutant's result: it explains, it doesn't judge.",
        takes_mutant_id: true,
        examples: &[
            "mutastic explain mb2f8ae57",
            "mutastic explain mb2f8ae57 fixtures/pricing",
        ],
    },
    Command {
        name: "ignore",
        page: "ignore",
        synopsis: "mutastic ignore ID [DIR] --reason=WHY",
        summary: "Record a decision not to test one mutant",
        detail: "It goes in .mutastic/ignores.yaml in the package's directory with its line and \
                 the reason, to commit and review, and lapses when that line changes: the mutant \
                 runs again, and run-finished lists it in staleIgnores. --remove takes one back; \
                 ignore --prune takes away every one that has lapsed. --propose records it in \
                 .mutastic/proposals.yaml instead, for a person to accept with --accept: how a \
                 coding agent says a mutant isn't worth a test, since until then it still counts \
                 as missed. A proposal that a mutant is equivalent or unreachable says the one case \
                 where it differs, and why that can't happen. It runs no tests, and each step says \
                 how to undo it.",
        exits: "0 when the decision is recorded, proposed, accepted or removed; 1 when --accept \
                names a mutant that isn't proposed, or --remove one that isn't ignored.",
        takes_mutant_id: true,
        examples: &[
            "mutastic ignore mb2f8ae57 --reason=\"totals are rounded to tens\"",
            "mutastic ignore mb2f8ae57 --propose --as=unreachable --reason=\"Differs only when total is 100. That can't happen because prices end in 9.\"",
            "mutastic ignore --accept mb2f8ae57",
            "mutastic ignore --remove mb2f8ae57",
            "mutastic ignore --prune",
        ],
    },
    Command {
        name: "next",
        page: "next",
        synopsis: "mutastic next [DIR] [OPTIONS]",
        summary: "Show the missed mutant most worth a test first, and why",
        detail: "The missed mutants as a queue, one line's at a time, in the order a run ranks \
                 them: its card, why it's first, and the command that says when it's done. --top \
                 shows the first few, spread over the code. With a baseline, new misses come before \
                 the baseline's. While the code under test is the same, \
                 it runs only the mutants the last next left uncaught, so any a new test catches \
                 drop out; once that code changes, it runs every mutant again. A mutant proposed \
                 for ignoring isn't queued: it waits on a person. For an agent, one task, and to \
                 run next again after it.",
        exits: "0 when nothing missed is left, or only mutants proposed for ignoring; 1 while \
                something is queued.",
        takes_mutant_id: false,
        examples: &[
            "mutastic next",
            "mutastic next --top=5",
            "mutastic next --format=agent",
        ],
    },
    Command {
        name: "baseline",
        page: "baseline",
        synopsis: "mutastic baseline [DIR]",
        summary: "Record every mutant missed now, so later misses are told from them",
        detail: "Runs the mutants and writes each one missed or not covered, by id, to \
                 .mutastic/baseline.yaml in the package's directory, to commit. From then on a \
                 miss that isn't in it is new: gate holds only new misses, next offers them first, \
                 and run --fail-on-new fails only on them. It's never rewritten to take in new \
                 misses: --prune runs the mutants and drops the baseline's that are caught now.",
        exits: "0 when the baseline is written or pruned; 1 when there's one already and --prune \
                isn't given, or --prune is given and there's none.",
        takes_mutant_id: false,
        examples: &["mutastic baseline", "mutastic baseline --prune"],
    },
    Command {
        name: "gate open",
        page: "gate-open",
        synopsis: "mutastic gate open [DIR]",
        summary: "Start gating a coding agent's task: keep where it starts",
        detail: "Keeps the commit the task starts from, its ignores, and how many tests run and \
                 are ignored, in target/mutastic, so gate check can say whether the task is done. \
                 With --hook, once for each of the agent's sessions; at a later prompt, it takes in \
                 the ignores made since the agent stopped.",
        exits: "0 once it has kept where the task starts; 2 outside a git repository with a \
                commit.",
        takes_mutant_id: false,
        examples: &["mutastic gate open", "mutastic gate open --hook=claude"],
    },
    Command {
        name: "gate check",
        page: "gate-check",
        synopsis: "mutastic gate check [DIR]",
        summary: "Say whether a gated task is done: its changed lines' mutants caught",
        detail: "Runs the tests and the mutants on the lines changed since gate open: the tests \
                 run even with no mutant on those lines, and only a task that changed nothing \
                 skips them. The task is done when the tests pass, each mutant is caught or \
                 proposed for a person to ignore, no ignore was made by the task, and no test was \
                 taken away or ignored; until then it says what's left, as the agent \
                 format does. With --hook it answers as the agent's hook, and lets the agent stop, \
                 saying what's left, when it changed nothing since it was sent back, or after \
                 --tries.",
        exits: "0 when the task is done, 1 when it isn't, 2 when no task is open. With --hook, 0 \
                either way: the answer is what it prints, a block or a message, as the agent's \
                hooks read it, and with nothing open the agent may stop.",
        takes_mutant_id: false,
        examples: &["mutastic gate check", "mutastic gate check --hook=claude"],
    },
    Command {
        name: "gate close",
        page: "gate-close",
        synopsis: "mutastic gate close [DIR]",
        summary: "Stop gating a task: forget where it started",
        detail: "The next gate open starts a new task.",
        exits: "0 whether a task was open or not: it says which, or with --hook says nothing.",
        takes_mutant_id: false,
        examples: &["mutastic gate close"],
    },
    Command {
        name: "mutants",
        page: "mutants",
        synopsis: "mutastic mutants [DIR]",
        summary: "List the mutants, without building or running anything",
        detail: "One line each: its id, place, operator, the change, and the function it's in. A \
                 mutant near unsafe code says why it's marked, and the last lines count what was \
                 left out. With --operators and --unsafe, it shows what those would add before a \
                 run.",
        exits: "0 once they're listed.",
        takes_mutant_id: false,
        examples: &[
            "mutastic mutants",
            "mutastic mutants fixtures/pricing --operators=stronger",
        ],
    },
];

pub const OPTIONS: &[CliOption] = &[
    CliOption {
        name: "threshold",
        value: "N",
        kind: Kind::Value,
        group: "Running",
        choices: &[],
        help: "The score a run must reach to pass, a whole number from 0 to 100; 80 unless set",
        commands: &["run"],
        environment: "",
        example: "--threshold=70",
    },
    CliOption {
        name: "fail-on-new",
        value: "",
        kind: Kind::Flag,
        group: "Running",
        choices: &[],
        help: "Pass when every mutant no test catches is in .mutastic/baseline.yaml, and fail on \
               one that isn't, whatever the score: the threshold isn't applied. For a package whose \
               score can't reach it yet; mutastic baseline writes the file",
        commands: &["run"],
        environment: "",
        example: "--fail-on-new",
    },
    CliOption {
        name: "since",
        value: "REF",
        kind: Kind::OptionalValue,
        group: "Running",
        choices: &[],
        help: "Mutate only the lines changed since the merge-base of HEAD and REF, uncommitted \
               and new files included; the default branch without one. With no mutant on those \
               lines, nothing is built or run",
        commands: &["run", "next"],
        environment: "",
        example: "--since=main",
    },
    CliOption {
        name: "only",
        value: "ID",
        kind: Kind::Value,
        group: "Running",
        choices: &[],
        help: "Run one mutant, or several split by commas, whatever --operators says",
        commands: &["run"],
        environment: "",
        example: "--only=mb2f8ae57",
    },
    CliOption {
        name: "workers",
        value: "N",
        kind: Kind::Value,
        group: "Running",
        choices: &[],
        help: "How many mutants run side by side; half the cores, from 1 to 4, unless set",
        commands: &["run", "verify", "explain", "next", "baseline"],
        environment: "",
        example: "--workers=2",
    },
    CliOption {
        name: "no-suggestions",
        value: "",
        kind: Kind::Flag,
        group: "Running",
        choices: &[],
        help: "Don't look for a test that catches each missed mutant, which runs the missed \
               mutant's function outside the tests",
        commands: &["run", "explain", "next"],
        environment: "",
        example: "--no-suggestions",
    },
    CliOption {
        name: "force-coverage",
        value: "",
        kind: Kind::Flag,
        group: "Running",
        choices: &[],
        help: "Build and run every test even when no mutant is there to run, to check or time \
               the suite",
        commands: &["run"],
        environment: "",
        example: "--since --force-coverage",
    },
    CliOption {
        name: "operators",
        value: "OPERATORS",
        kind: Kind::Value,
        group: "Running",
        choices: &[],
        help: "Which operators run: a set (defaults, stronger or extended), or operator ids split \
               by commas, in place of mutastic.yaml's operators; defaults unless set",
        commands: &["run", "mutants", "next", "baseline"],
        environment: "",
        example: "--operators=stronger",
    },
    CliOption {
        name: "unsafe",
        value: "MODE",
        kind: Kind::Value,
        group: "Running",
        choices: &["skip", "enclosing", "all"],
        help: "Which unsafe code is mutated: skip leaves unsafe code and the functions holding \
               an unsafe block alone, enclosing mutates those functions too, all unsafe code as \
               well; skip unless set",
        commands: &["run", "mutants", "next", "baseline"],
        environment: "",
        example: "--unsafe=enclosing",
    },
    CliOption {
        name: "report",
        value: "KIND[:PATH]",
        kind: Kind::Repeated,
        group: "Running",
        choices: &["json", "html", "sarif", "none"],
        help: "A report file a full run writes: json (mutation.json), html (mutation.html) or \
               sarif (mutation.sarif), in target/mutastic unless a path is given, or none; json and \
               html unless set",
        commands: &["run"],
        environment: "",
        example: "--report=json",
    },
    CliOption {
        name: "shard",
        value: "N/M",
        kind: Kind::Value,
        group: "Running",
        choices: &[],
        help: "Run only the Nth of M shares of the mutants, to split one run across machines. It \
               keeps its results in shares/N-of-M in target/mutastic for --from-shards",
        commands: &["run"],
        environment: "",
        example: "--shard=2/4 --threshold=0",
    },
    CliOption {
        name: "from-shards",
        value: "DIR",
        kind: Kind::Value,
        group: "Running",
        choices: &[],
        help: "Run no mutants: put together the results every share of a split run kept, found \
               anywhere in DIR, as the whole run, with its reports and threshold",
        commands: &["run"],
        environment: "",
        example: "--from-shards=shares --threshold=70",
    },
    CliOption {
        name: "format",
        value: "FORMAT",
        kind: Kind::Value,
        group: "Output",
        choices: &["full", "brief", "concise", "grouped", "agent", "jsonl"],
        help: "How the result is shown; full on a terminal, concise elsewhere, agent for coding \
               agents; explain takes full and jsonl",
        commands: &["run", "next", "explain"],
        environment: "MUTASTIC_FORMAT, MUTASTIC_AGENT",
        example: "--format=jsonl",
    },
    CliOption {
        name: "vocabulary",
        value: "WORDS",
        kind: Kind::Value,
        group: "Output",
        choices: &["plain", "classic"],
        help: "plain says caught and missed; classic says killed and survived",
        commands: &["run", "explain", "next"],
        environment: "MUTASTIC_VOCABULARY",
        example: "--vocabulary=classic",
    },
    CliOption {
        name: "color",
        value: "WHEN",
        kind: Kind::Value,
        group: "Output",
        choices: &["auto", "always", "never"],
        help: "Colour: auto uses it on a terminal that wants it",
        commands: &["run", "explain", "next"],
        environment: "NO_COLOR, FORCE_COLOR",
        example: "--color=never",
    },
    CliOption {
        name: "hyperlinks",
        value: "WHEN",
        kind: Kind::Value,
        group: "Output",
        choices: &["auto", "always", "never"],
        help: "File locations as links a terminal can open: auto uses them on a terminal",
        commands: &["run", "explain", "next"],
        environment: "",
        example: "--hyperlinks=never",
    },
    CliOption {
        name: "progress",
        value: "STYLE",
        kind: Kind::Value,
        group: "Output",
        choices: &["auto", "bar", "counter", "none"],
        help: "Progress on stderr: redrawn in place on a terminal, a line every 30 s in logs, CI and \
               agents' runs, or none",
        commands: &["run", "verify", "explain", "next", "baseline"],
        environment: "MUTASTIC_PROGRESS",
        example: "--progress=counter",
    },
    CliOption {
        name: "top",
        value: "N",
        kind: Kind::Value,
        group: "Output",
        choices: &[],
        help: "How many of the queue next shows, at most one a function and two a file; 1 unless \
               set, at most 25",
        commands: &["next"],
        environment: "",
        example: "mutastic next --top=5",
    },
    CliOption {
        name: "reason",
        value: "WHY",
        kind: Kind::Value,
        group: "Ignoring",
        choices: &[],
        help: "Why the mutant isn't worth a test: every ignore says",
        commands: &["ignore"],
        environment: "",
        example: "--reason=\"totals are rounded to tens\"",
    },
    CliOption {
        name: "as",
        value: "KIND",
        kind: Kind::Value,
        group: "Ignoring",
        choices: crate::ignores::KINDS,
        help: "What sort of decision it is",
        commands: &["ignore"],
        environment: "",
        example: "--as=unreachable --reason=\"totals are rounded to tens\"",
    },
    CliOption {
        name: "remove",
        value: "",
        kind: Kind::Flag,
        group: "Ignoring",
        choices: &[],
        help: "Take the mutant's ignore, or its proposal, away, so it runs again",
        commands: &["ignore"],
        environment: "",
        example: "mutastic ignore --remove mb2f8ae57",
    },
    CliOption {
        name: "propose",
        value: "",
        kind: Kind::Flag,
        group: "Ignoring",
        choices: &[],
        help: "Propose the ignore for a person to accept, rather than make it: a coding agent's \
               way. An equivalent or unreachable mutant's reason says the one case where it \
               differs and why that can't happen",
        commands: &["ignore"],
        environment: "",
        example:
            "--propose --as=unreachable --reason=\"Differs only when total is 100. That can't \
                  happen because prices end in 9.\"",
    },
    CliOption {
        name: "accept",
        value: "",
        kind: Kind::Flag,
        group: "Ignoring",
        choices: &[],
        help: "Accept a proposed ignore: it moves into the ignores, as proposed",
        commands: &["ignore"],
        environment: "",
        example: "mutastic ignore --accept mb2f8ae57",
    },
    CliOption {
        name: "prune",
        value: "",
        kind: Kind::Flag,
        group: "Ignoring",
        choices: &[],
        help: "Take away every ignore whose line has changed; with baseline, every mutant the \
               tests catch now",
        commands: &["ignore", "baseline"],
        environment: "",
        example: "--prune",
    },
    CliOption {
        name: "hook",
        value: "AGENT",
        kind: Kind::Value,
        group: "Gate",
        choices: &["claude"],
        help: "Run as the agent's hook: read its input on stdin, and answer as its hooks do",
        commands: &["gate open", "gate check", "gate close"],
        environment: "",
        example: "--hook=claude",
    },
    CliOption {
        name: "tries",
        value: "N",
        kind: Kind::Value,
        group: "Gate",
        choices: &[],
        help: "With --hook, how many times check sends the agent back before it lets it stop and \
               says what's left, from 1 to 100; 3 unless set",
        commands: &["gate check"],
        environment: "",
        example: "mutastic gate check --hook=claude --tries=5",
    },
    CliOption {
        name: "edition",
        value: "EDITION",
        kind: Kind::Value,
        group: "Editions",
        choices: &["rust", "java"],
        help: "Which edition runs; unless set, the build file in DIR, or the nearest above it: \
               Cargo.toml for rust, pom.xml, build.gradle or settings.gradle for java",
        commands: EVERY_COMMAND,
        environment: "",
        example: "--edition=rust",
    },
];

pub const ENVIRONMENT: &[Entry] = &[
    Entry {
        name: "MUTASTIC_FORMAT",
        meaning: "The format when --format isn't given",
        detail: "For a machine or a CI job that always wants the same format, `concise` say. A \
                 format mutastic doesn't have is refused, as the option's is.",
    },
    Entry {
        name: "MUTASTIC_AGENT=1",
        meaning: "The agent format, for coding agents",
        detail: "Coding agents set it, so mutastic answers in the format they read best, with \
                 progress as lines. `MUTASTIC_FORMAT` still wins.",
    },
    Entry {
        name: "MUTASTIC_VOCABULARY",
        meaning: "plain or classic, when --vocabulary isn't given",
        detail: "`classic` says killed and survived instead of caught and missed. The JSON report \
                 and the event stream always use the standard names. Any other value is refused.",
    },
    Entry {
        name: "NO_COLOR",
        meaning: "No colour",
        detail: "Set to anything, it turns colour off, as it does for many tools.",
    },
    Entry {
        name: "FORCE_COLOR",
        meaning: "Colour even off a terminal",
        detail: "Set to anything, it turns colour on where mutastic can't tell a person is \
                 reading, such as a CI log that shows colour. It wins over `NO_COLOR`.",
    },
    Entry {
        name: "TERM=dumb",
        meaning: "No colour, unless `FORCE_COLOR` is set, and progress as lines",
        detail: "",
    },
    Entry {
        name: "MUTASTIC_PROGRESS",
        meaning: "auto, bar, counter or none, when --progress isn't given",
        detail: "For a Gradle or Maven build, or a CI job, that always wants the same. Any other \
                 value is refused.",
    },
    Entry {
        name: "MUTASTIC_TERM_PROGRESS",
        meaning: "true or false: whether the terminal's tab shows the run's progress",
        detail: "Unset, it does in Windows Terminal, ConEmu, Ghostty and WezTerm, which say they \
                 draw it. Cargo's `CARGO_TERM_PROGRESS_TERM_INTEGRATION` is read too.",
    },
    Entry {
        name: "MUTASTIC_HEARTBEAT",
        meaning: "Seconds a log may go without a progress line",
        detail: "30 unless set. Off a terminal, a line with the phase, its counts and its clock is \
                 printed whenever this long has passed since the last. A value that isn't a number \
                 above 0 is refused.",
    },
    Entry {
        name: "CI",
        meaning: "Progress as lines, not redrawn in place",
        detail: "Set to anything but `false` or `0`; as are `TF_BUILD`, `GITHUB_ACTIONS`, \
                 `GITLAB_CI`, `BUILDKITE`, `JENKINS_URL`, `TEAMCITY_VERSION` and `CIRCLECI`, \
                 since some CI services pretend to be a terminal.",
    },
    Entry {
        name: "CLAUDECODE",
        meaning: "Progress as lines, for a coding agent",
        detail: "As are `CLAUDE_CODE`, `GEMINI_CLI`, `CODEX_SANDBOX`, `CODEX_THREAD_ID`, \
                 `OPENCODE`, `COPILOT_AGENT`, `COPILOT_CLI`, `CURSOR_AGENT`, `REPL_ID`, \
                 `JUNIE_DATA`, `GOOSE_PROVIDER` and `AUGMENT_AGENT`, which coding agents set. \
                 Only progress changes: the format is still `--format`'s.",
    },
    Entry {
        name: "CARGO",
        meaning: "The cargo mutastic runs",
        detail: "Cargo sets it for `cargo mutastic`; `cargo` on the `PATH` otherwise.",
    },
    Entry {
        name: "RUSTC",
        meaning: "The rustc Cargo builds with, as for any Cargo build",
        detail: "The result names its version.",
    },
    Entry {
        name: "MUTASTIC_VERSION",
        meaning: "In a Maven or Gradle project, the Java edition's release to use",
        detail: "Such as `0.5.0`, fetched if it isn't kept. Unset, from 1.0.0 it's the newest release of this mutastic's own major and minor version.",
    },
    Entry {
        name: "MUTASTIC_HOME",
        meaning: "Where the Java edition's releases are kept",
        detail: "`~/.mutastic` unless set.",
    },
    Entry {
        name: "MUTASTIC_STDERR_TTY",
        meaning: "Whether stderr is a terminal, 1 or 0, for the Java edition",
        detail: "The JDK can't tell, so `mutastic` sets it when it runs the Java edition, unless \
                 it's set already.",
    },
    Entry {
        name: "MUTASTIC_COLUMNS",
        meaning: "The terminal's width, for the Java edition",
        detail: "The JDK can't ask the terminal, so `mutastic` sets it when it runs the Java \
                 edition on a terminal, unless it's set already.",
    },
    Entry {
        name: "MUTASTIC_JAVA_ENGINE",
        meaning:
            "A Java edition's `bin/mutastic` to run instead, such as one built from its source",
        detail: "Nothing is fetched.",
    },
    Entry {
        name: "MUTASTIC_JAVA_RELEASES",
        meaning: "Where the Java edition's releases are downloaded from, for a mirror",
        detail: "GitHub's releases of `mutastic/mutastic` unless set.",
    },
    Entry {
        name: "JAVA_HOME",
        meaning: "The JDK the Java edition runs on",
        detail: "`java` on the `PATH` unless set.",
    },
];

pub const EXIT_CODES: &[Entry] = &[
    Entry {
        name: "0",
        meaning: "The score reaches the threshold, or with --fail-on-new no miss is new; for verify, each mutant named is caught, \
                  skipped, or crashed where a crash proves nothing; for gate check, the task is \
                  done; for next, nothing missed is left but what's proposed for ignoring",
        detail: "",
    },
    Entry {
        name: "1",
        meaning: "The score is below the threshold, or with --fail-on-new a miss is new; for verify, a mutant named is still missed or \
                  not covered; for gate check, the task isn't done; for ignore --accept or --remove, \
                  the mutant isn't proposed or ignored; for next, something is queued; for baseline, there's one \
                  already, or none to prune",
        detail: "",
    },
    Entry {
        name: "2",
        meaning: "mutastic couldn't run: an unknown option, a word the command doesn't take, not \
                  a Cargo package, a workspace's root, a test that fails with no mutant in, an id \
                  that isn't a mutant of the package, shares --from-shards can't put together as one \
                  run, --fail-on-new with no baseline, or no task open for gate check; or, for run \
                  and verify, a test that caught mutants fails on its own, with no mutant in",
        detail: "",
    },
];

/// The command declared by that name: `run`, `gate open`.
pub fn command(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.name == name)
}

/// The option declared by that name, without its dashes.
pub fn option(name: &str) -> Option<&'static CliOption> {
    OPTIONS.iter().find(|option| option.name == name)
}

/// The options `command` takes, in the order they're declared.
pub fn options_for(command: &str) -> Vec<&'static CliOption> {
    OPTIONS
        .iter()
        .filter(|option| option.commands.contains(&command))
        .collect()
}

/// Whether `command` takes `--name`, given `value` (`None` without an `=`): an option mutastic
/// doesn't declare, one this command doesn't take, or a flag given a value is an error.
pub fn accepts(command: Option<&Command>, name: &str, value: Option<&str>) -> Result<(), Failure> {
    let Some(option) = option(name) else {
        return Err(Code::UnknownOption.with(format!("unknown option --{name}")));
    };
    if let Some(command) = command {
        if !option.commands.contains(&command.name) {
            return Err(Code::OptionNotTaken.with(format!(
                "{} doesn't take --{name}: {}",
                command.name, command.synopsis
            )));
        }
    }
    if option.kind == Kind::Flag && value.is_some() {
        return Err(Code::OptionValue.with(format!("--{name} takes no value")));
    }
    Ok(())
}

/// `commands.json`: the commands and options as the docs site's reference pages read them, in the
/// Java edition's shape. Each example is written for the two ways to run mutastic, `mutastic` and
/// `cargo mutastic`; the Java edition's Maven and Gradle runners have no counterpart here.
pub fn model() -> Value {
    json!({
        "schema": SCHEMA,
        "docs": DOCS,
        "commands": COMMANDS.iter().map(command_model).collect::<Vec<_>>(),
        "options": OPTIONS.iter().map(option_model).collect::<Vec<_>>(),
        "environment": ENVIRONMENT.iter().map(entry_model).collect::<Vec<_>>(),
        "exitCodes": EXIT_CODES.iter().map(entry_model).collect::<Vec<_>>(),
    })
}

/// A command line, as each runner writes it.
fn runners(command_line: &str) -> Value {
    json!({ "commandLine": command_line, "cargo": format!("cargo {command_line}") })
}

fn command_model(command: &Command) -> Value {
    let options: Vec<String> = options_for(command.name)
        .into_iter()
        .map(|option| format!("--{}", option.name))
        .collect();
    json!({
        "name": command.name,
        "page": command.page,
        "synopsis": command.synopsis,
        "summary": command.summary,
        "detail": command.detail,
        "exits": command.exits,
        "takesMutantId": command.takes_mutant_id,
        "examples": command.examples.iter().copied().map(runners).collect::<Vec<_>>(),
        "options": options,
    })
}

fn option_model(option: &CliOption) -> Value {
    let mut commands: Vec<&Command> = option
        .commands
        .iter()
        .filter_map(|name| command(name))
        .collect();
    commands.sort_by_key(|command| command.name);
    let examples: serde_json::Map<String, Value> = commands
        .iter()
        .map(|command| {
            (
                command.name.to_string(),
                runners(&option.example_for(command)),
            )
        })
        .collect();
    json!({
        "name": format!("--{}", option.name),
        "synopsis": option.synopsis(),
        "kind": option.kind.name(),
        "group": option.group,
        "value": option.value,
        "choices": option.choices,
        "help": option.help,
        "commands": commands.iter().map(|command| command.name).collect::<Vec<_>>(),
        "environment": option.environment,
        "examples": examples,
    })
}

fn entry_model(entry: &Entry) -> Value {
    json!({ "name": entry.name, "meaning": entry.meaning, "detail": entry.detail })
}

/// `commands.json` as it's kept in `docs/`: two-space indents and a final newline.
pub fn json() -> String {
    let mut text = serde_json::to_string_pretty(&model()).unwrap_or_default();
    text.push('\n');
    text
}

/// `docs/reference/commands.md`: the same declarations as `commands.json`, as a page GitHub shows,
/// so the repository's own docs can link to a command (`commands.md#run`), an option
/// (`commands.md#--threshold`), the environment and the exit codes. The docs site renders
/// `commands.json` instead, and maps those links to its own pages.
pub fn markdown() -> String {
    let mut page = String::from(
        "# Commands\n\n\
         <!-- Written from crates/mutastic/src/commands.rs by the docs test, with \
         MUTASTIC_DOCS_WRITE=1 cargo test --test docs; change the declarations, not this page. -->\n\n\
         Every command `mutastic` takes, with its options, the environment it reads and the codes it \
         exits with. `cargo mutastic` takes the same words.\n\n",
    );
    for command in COMMANDS {
        page.push_str(&format!(
            "- [`mutastic {}`](#{}): {}\n",
            command.name, command.page, command.summary
        ));
    }
    page.push_str("- [Environment](#environment) and [exit codes](#exit-codes)\n");
    for command in COMMANDS {
        page.push_str(&command_markdown(command));
    }
    page.push_str("\n## Options\n");
    for option in OPTIONS {
        page.push_str(&option_markdown(option));
    }
    page.push_str("\n## Environment\n\n");
    page.push_str(&entries_markdown(ENVIRONMENT, true));
    page.push_str("\n## Exit codes\n\n");
    page.push_str(&entries_markdown(EXIT_CODES, false));
    page
}

/// A command's section, headed by its name so its anchor is its page on the docs site: `gate-open`.
fn command_markdown(command: &Command) -> String {
    let options: Vec<String> = options_for(command.name)
        .into_iter()
        .map(|option| format!("[`{}`](#--{})", option.synopsis(), option.name))
        .collect();
    let listed = if options.is_empty() {
        String::new()
    } else {
        format!("**Options:** {}.\n\n", options.join(", "))
    };
    format!(
        "\n## {}\n\n`{}`\n\n{}. {}\n\n```sh\n{}\n```\n\n{listed}**Exit status:** {}\n",
        command.name,
        command.synopsis,
        command.summary,
        command.detail,
        command.examples.join("\n"),
        command.exits
    )
}

fn option_markdown(option: &CliOption) -> String {
    let mut commands: Vec<&Command> = option
        .commands
        .iter()
        .filter_map(|name| command(name))
        .collect();
    commands.sort_by_key(|command| command.name);
    let taken: Vec<String> = commands
        .iter()
        .map(|command| format!("[`{}`](#{})", command.name, command.page))
        .collect();
    let choices = if option.choices.is_empty() {
        String::new()
    } else {
        let each: Vec<String> = option.choices.iter().map(|c| format!("`{c}`")).collect();
        format!(" One of {}.", each.join(", "))
    };
    let environment = if option.environment.is_empty() {
        String::new()
    } else {
        let each: Vec<String> = option
            .environment
            .split(", ")
            .map(|name| format!("`{name}`"))
            .collect();
        format!(" Also set by {}.", each.join(" or "))
    };
    let examples: Vec<String> = commands
        .iter()
        .map(|command| option.example_for(command))
        .collect();
    format!(
        "\n### `--{}`\n\n`{}`, for {}. {}.{choices}{environment}\n\n```sh\n{}\n```\n",
        option.name,
        option.synopsis(),
        taken.join(", "),
        option.help,
        examples.join("\n")
    )
}

fn entries_markdown(entries: &[Entry], code: bool) -> String {
    entries
        .iter()
        .map(|entry| {
            let name = if code {
                format!("`{}`", entry.name)
            } else {
                format!("**{}**", entry.name)
            };
            let detail = if entry.detail.is_empty() {
                String::new()
            } else {
                format!(" {}", entry.detail)
            };
            format!("- {name}: {}.{detail}\n", entry.meaning)
        })
        .collect()
}
