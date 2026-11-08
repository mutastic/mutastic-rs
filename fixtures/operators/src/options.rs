//! `Option` and `Result`.

pub fn display_name(name: &str, nickname: Option<&str>) -> String {
    if nickname.is_none() {
        return name.to_string();
    }
    nickname.unwrap_or(name).to_string()
}

/// 20 to a page unless a size is chosen.
pub fn page_size(chosen: u32) -> u32 {
    let custom = if chosen > 0 { Some(chosen) } else { None };
    custom.unwrap_or(20)
}

pub fn port(configured: Option<u16>) -> u16 {
    configured.unwrap_or(8080)
}

pub fn plausible_age(age: Option<u32>) -> Option<u32> {
    age.filter(|years| *years < 150)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_nickname() {
        assert_eq!(display_name("Ann", None), "Ann");
    }

    #[test]
    fn a_chosen_page_size() {
        assert_eq!(page_size(20), 20);
    }

    #[test]
    fn the_configured_port() {
        assert_eq!(port(Some(8080)), 8080);
    }

    #[test]
    fn an_ordinary_age() {
        assert_eq!(plausible_age(Some(30)), Some(30));
    }
}
