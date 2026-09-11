//! Compile-time contracts for the generated public API and schema grammar.

#![cfg(not(miri))]

/// Set `PINAPOD_UI=skip` on toolchains whose diagnostics differ from the
/// pinned nightly that records the `.stderr` snapshots, such as the stable
/// and MSRV verification jobs. Compile-pass contracts always run; only the
/// snapshot-matched failure cases are skipped.
#[test]
fn generated_api_and_schema_contracts() {
    let cases = trybuild::TestCases::new();

    cases.pass("tests/ui/pass/*.rs");

    if std::env::var_os("PINAPOD_UI").is_some_and(|value| value.eq_ignore_ascii_case("skip")) {
        eprintln!("skipping UI snapshots: PINAPOD_UI=skip");
        return;
    }

    cases.compile_fail("tests/ui/fail/*.rs");
}
