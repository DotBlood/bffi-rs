//! Compile-failure acceptance tests (kanboard 7.2): every rejection must
//! produce the exact documented message.

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
