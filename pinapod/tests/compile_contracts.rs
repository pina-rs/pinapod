//! Compile-time contracts for the generated public API and schema grammar.

#![cfg(not(miri))]

/// Set `PINAPOD_UI=skip` on toolchains whose diagnostics differ from the
/// pinned nightly that records the `.stderr` snapshots, such as the stable
/// and MSRV verification jobs. The snapshots still run on the nightly job.
#[test]
fn generated_api_and_schema_contracts() {
    if std::env::var_os("PINAPOD_UI").is_some_and(|value| value.eq_ignore_ascii_case("skip")) {
        eprintln!("skipping UI snapshots: PINAPOD_UI=skip");
        return;
    }

    let cases = trybuild::TestCases::new();

    cases.pass("tests/ui/pass/*.rs");
    cases.compile_fail("tests/ui/fail/*.rs");
}
