//! trybuild UI tests for the class diagnostics (E005-E008). The
//! `.stderr` goldens pin the exact `bffi[E0XX]:` format - do not
//! renumber; regenerate with `TRYBUILD=overwrite cargo test --test ui`
//! and review by hand.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
