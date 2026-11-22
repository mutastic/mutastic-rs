//! What an audit of the arid rules found (`MUTASTIC_ARID=audit`), as the Java edition says it: the
//! mutants they skip, run anyway. A catch by an assertion means a test checks what the arid code
//! does, so a rule may be hiding a contract; a catch by a panic in the code (an `unwrap` on a value
//! the mutant changed) only means the code is used, which says nothing either way.

use serde_json::{json, Value};

use crate::engine::{MutantResult, Report, Status};

/// How many functions the summary names, and gives a rule for.
const SHOWN: usize = 3;

pub struct AridAudit {
    pub ran: usize,
    pub caught: usize,
    pub by_assertion: usize,
    pub missed: usize,
    pub not_covered: usize,
    /// The functions of the catches by an assertion, `Money::add()`, in the order found.
    pub caught_in: Vec<String>,
    /// For each of those, the `mutastic.yaml` rule that runs the arid rule's mutants there: its
    /// `- mutate:` line, its `arid:` and `reason:` under it.
    pub bring_back: Vec<String>,
}

impl AridAudit {
    /// The audit of a run that was one; `None` for any other.
    pub fn of(report: &Report) -> Option<AridAudit> {
        if report.audited.is_empty() {
            return None;
        }
        let audited = &report.audited;
        let checked: Vec<&MutantResult> = audited
            .iter()
            .filter(|r| r.status.detected() && r.by_assertion)
            .collect();
        let count = |status: Status| audited.iter().filter(|r| r.status == status).count();
        let mut caught_in: Vec<String> = Vec::new();
        let mut bring_back: Vec<String> = Vec::new();
        for result in &checked {
            let mutant = &report.found.mutants[result.mutant];
            let function = &report.found.functions[mutant.function].path;
            let name = format!("{function}()");
            if !caught_in.contains(&name) {
                caught_in.push(name);
            }
            // The run's own result for it has its reason: the audited one ran without.
            let rule = mutant.skipped.as_deref().and_then(crate::arid::rule);
            if let Some(rule) = rule {
                let text = rule.bring_back(function);
                let rule = text
                    .split_once('\n')
                    .map_or("", |(_, rest)| rest)
                    .to_string();
                if !bring_back.contains(&rule) {
                    bring_back.push(rule);
                }
            }
        }
        Some(AridAudit {
            ran: audited.len(),
            caught: audited.iter().filter(|r| r.status.detected()).count(),
            by_assertion: checked.len(),
            missed: count(Status::Survived),
            not_covered: count(Status::NoCoverage),
            caught_in,
            bring_back,
        })
    }

    /// What to put in `mutastic.yaml` to run the arid mutants a test checks, in the first
    /// functions the summary names: a `rules:` list, a line each.
    pub fn rules(&self) -> Vec<String> {
        if self.bring_back.is_empty() {
            return Vec::new();
        }
        let mut lines = vec!["rules:".to_string()];
        lines.extend(
            self.bring_back
                .iter()
                .take(SHOWN)
                .flat_map(|rule| rule.lines().map(str::to_string)),
        );
        lines
    }

    /// `12 ran anyway: 3 caught (1 by an assertion), 8 missed, 1 not covered; checked in
    /// Money::add()`.
    pub fn summary(&self) -> String {
        let counts = format!(
            "{} ran anyway: {} caught ({} by an assertion), {} missed, {} not covered",
            self.ran, self.caught, self.by_assertion, self.missed, self.not_covered
        );
        if self.caught_in.is_empty() {
            return counts;
        }
        let shown = &self.caught_in[..self.caught_in.len().min(SHOWN)];
        let more = self.caught_in.len() - shown.len();
        format!(
            "{counts}; checked in {}{}",
            shown.join(", "),
            if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            }
        )
    }

    /// The summary's row: the counts, and the rule that runs them where a test checks them.
    pub fn row(&self) -> String {
        let rules = self.rules();
        if rules.is_empty() {
            self.summary()
        } else {
            format!(
                "{}\na test checks what they do: to run them, add to mutastic.yaml\n  {}",
                self.summary(),
                rules.join("\n  ")
            )
        }
    }

    /// `run-finished`'s `aridAudit`.
    pub fn event(&self) -> Value {
        let mut found = json!({
            "ran": self.ran,
            "caught": self.caught,
            "caughtByAssertion": self.by_assertion,
            "missed": self.missed,
            "notCovered": self.not_covered,
            "checkedIn": self.caught_in,
        });
        let rules = self.rules();
        if !rules.is_empty() {
            found["bringBack"] = json!(format!("{}\n", rules.join("\n")));
        }
        found
    }
}
