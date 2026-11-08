//! What a function returns, and the errors it gives.

/// A point for every 10 spent.
pub fn loyalty_points(spent: u32) -> u32 {
    spent / 10
}

/// Six digits.
pub fn is_valid_code(code: &str) -> bool {
    code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
}

pub fn parse_quantity(text: &str) -> Result<u32, std::num::ParseIntError> {
    let quantity: u32 = text.trim().parse()?;
    Ok(quantity)
}

pub fn quantity(text: &str) -> Result<u32, String> {
    if text.is_empty() {
        return Err("no quantity given".into());
    }
    text.parse().map_err(|_| format!("{text} isn't a number"))
}

/// A count of pieces, as a receipt shows it.
pub struct Pieces(pub u32);

impl std::fmt::Display for Pieces {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)?;
        f.write_str(" pcs")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_spends_earn_nothing() {
        assert_eq!(loyalty_points(5), 0);
    }

    #[test]
    fn a_good_code() {
        assert!(is_valid_code("123456"));
    }

    #[test]
    fn spaces_are_ignored() {
        assert_eq!(parse_quantity(" 3 "), Ok(3));
    }

    #[test]
    fn an_empty_quantity_is_an_error() {
        assert!(quantity("").is_err());
    }

    #[test]
    fn pieces_show_their_unit() {
        assert_eq!(Pieces(3).to_string(), "3 pcs");
    }
}
