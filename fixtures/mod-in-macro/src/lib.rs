//! A crate whose modules are declared by a macro of its own, as serde's are by `crate_root!()`:
//! rustc reads their files as if the `mod` were written where the macro is called.

macro_rules! crate_root {
    () => {
        pub mod inner;
    };
}

crate_root!();
