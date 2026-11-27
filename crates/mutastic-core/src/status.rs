//! A mutant's fate, as every edition names it: in the mutation-testing report schema, in the
//! events, and in what `verify` judges.

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    Killed,
    Survived,
    Timeout,
    NoCoverage,
    Ignored,
    /// Crashed where the crash may be undefined behaviour, or where std's own debug check of an
    /// unsafe precondition stopped it: no test can be said to have caught it. Out of the score.
    RuntimeError,
}

impl Status {
    /// The mutation-testing report schema's name.
    pub fn schema_name(self) -> &'static str {
        match self {
            Status::Killed => "Killed",
            Status::Survived => "Survived",
            Status::Timeout => "Timeout",
            Status::NoCoverage => "NoCoverage",
            Status::Ignored => "Ignored",
            Status::RuntimeError => "RuntimeError",
        }
    }

    /// The events format's name for a count of this status in `run-finished`:
    /// `killed`, `noCoverage`.
    pub fn event_name(self) -> &'static str {
        match self {
            Status::Killed => "killed",
            Status::Survived => "survived",
            Status::Timeout => "timeout",
            Status::NoCoverage => "noCoverage",
            Status::Ignored => "ignored",
            Status::RuntimeError => "runtimeError",
        }
    }

    /// The status a schema name stands for: `Killed` → [`Status::Killed`].
    pub fn from_schema_name(name: &str) -> Option<Status> {
        [
            Status::Killed,
            Status::Survived,
            Status::Timeout,
            Status::NoCoverage,
            Status::Ignored,
            Status::RuntimeError,
        ]
        .into_iter()
        .find(|status| status.schema_name() == name)
    }

    pub fn detected(self) -> bool {
        matches!(self, Status::Killed | Status::Timeout)
    }

    /// Whether a mutant asked for by id passes, as `verify` judges it: caught, skipped, or crashed
    /// where a crash proves nothing, which leaves nothing for a test to catch.
    pub fn settled(self) -> bool {
        self.detected() || matches!(self, Status::Ignored | Status::RuntimeError)
    }
}
