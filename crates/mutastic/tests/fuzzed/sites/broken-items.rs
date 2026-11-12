unsafe impl Send for {} fn g(x: NonZeroU32) { x.get() == 0 ; ? ; }
macro_rules! m { ($e:expr) => { $e >= 1 } }
fn h() { m!(); r#"
