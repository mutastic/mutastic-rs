//! A ledger: amounts kept in a file, one to a line, and the fee on each.

use std::io::Write;
use std::path::PathBuf;

/// The fee on an amount, a whole percent of it.
pub fn fee(amount: i64, percent: i64) -> i64 {
    amount * percent / 100
}

/// No fee on an amount at or over the threshold.
pub fn waived(amount: i64, threshold: i64) -> bool {
    amount >= threshold
}

/// Amounts kept in a file, one to a line.
pub struct Ledger {
    file: PathBuf,
}

impl Ledger {
    pub fn new(file: impl Into<PathBuf>) -> Ledger {
        Ledger { file: file.into() }
    }

    pub fn record(&self, amount: i64) -> std::io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file)?;
        writeln!(file, "{amount}")
    }

    pub fn total(&self) -> std::io::Result<i64> {
        if !self.file.exists() {
            return Ok(0);
        }
        let text = std::fs::read_to_string(&self.file)?;
        Ok(text.lines().filter_map(|line| line.parse::<i64>().ok()).sum())
    }

    pub fn is_empty(&self) -> std::io::Result<bool> {
        Ok(self.total()? == 0)
    }
}
