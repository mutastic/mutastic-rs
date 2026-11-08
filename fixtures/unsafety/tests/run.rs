use unsafety::{plain, raw};

#[test]
fn halves() {
    assert_eq!(raw::halve(4), 2);
}

#[test]
fn checks() {
    assert_eq!(raw::checked(3), 3);
}

#[test]
fn firsts() {
    assert_eq!(unsafe { raw::first(&[1]) }, 2);
}

#[test]
fn guards() {
    assert_eq!(plain::guarded(3), 3);
}

#[test]
fn totals() {
    assert_eq!(plain::total(1, 2), 3);
}
