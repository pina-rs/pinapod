//! Compile-time contracts for the generated public API and schema grammar.

#![cfg(not(miri))]

#[test]
fn generated_api_and_schema_contracts() {
    let cases = trybuild::TestCases::new();

    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}
