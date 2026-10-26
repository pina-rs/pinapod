{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:
let
  currentDir = builtins.dirOf __curPos.file;

  custom = inputs.ifiokjr-nixpkgs.packages.${pkgs.stdenv.hostPlatform.system};
  # Kani's prebuilt driver links the compiler libraries of one exact nightly
  # release, so the proof environment ships that toolchain alongside it. The
  # date is not free: it must be the nightly named by the packaged Kani's own
  # `rust-toolchain-version` file, or `kani-compiler` aborts on start with a
  # dyld "Library not loaded: @rpath/librustc_driver-<hash>.dylib" error.
  # Read it when bumping Kani's nixpkgs pin with:
  #   cat "$(nix build --no-link --print-out-paths \
  #     --impure --expr 'inputs.ifiokjr-nixpkgs.packages.<system>.kani')/rust-toolchain-version"
  kaniToolchain = pkgs.rust-bin.nightly."2026-08-21".minimal;
  kani = custom.kani.overrideAttrs (old: {
    buildInputs = (old.buildInputs or [ ]) ++ lib.optionals pkgs.stdenv.isLinux [ kaniToolchain ];
    postInstall = (old.postInstall or "") + ''
      if [ ! -e "$out/toolchain" ]; then
        ln -s ${kaniToolchain} "$out/toolchain"
      fi
    '';
  });
  # Kani preprocesses every harness by shelling out to a native C compiler, and
  # CBMC's `goto-cc` looks for the literal name `gcc`. A Darwin machine has no
  # `gcc` binary at all (Apple's is a clang stub, and clang is what the profile
  # ships), so goto-cc dies with "execvp gcc failed: No such file or directory"
  # before any proof runs. `--native-compiler clang` cannot be passed through
  # cargo-kani, so the shim supplies the name goto-cc insists on. It only has
  # to preprocess; goto-cc parses the C and never links the result.
  #
  # Darwin-only: Linux ships a real gcc, and shadowing it would be a surprise.
  kaniGccShim = lib.optionals pkgs.stdenv.isDarwin [
    (pkgs.writeShellScriptBin "gcc" ''
      exec ${pkgs.clang}/bin/clang "$@"
    '')
  ];
in
{
  packages = with pkgs; [
    bun
    cargo-audit
    cargo-deny
    cargo-llvm-cov
    dprint
    gh
    git
    gitleaks
    jq
    mdbook
    custom.mdt
    custom.monochange
    deno
    nixfmt-rfc-style
    rustup
    shfmt
    zizmor
  ];

  env.CARGO_TERM_COLOR = "always";

  # Cargo otherwise sizes its job pool to the whole machine, and several
  # scripts here fan out (clippy --fix re-runs the compiler to a fixpoint, and
  # the docs and test scripts each rebuild the workspace). Letting every cargo
  # invocation claim all cores at once is what makes a local `fix:all` or a
  # parallel Kani shard pin the CPU and start thrashing. Cap the pool instead;
  # override with CARGO_BUILD_JOBS=<n> for a one-off full-speed build.
  #
  # Deliberately not done via RUSTFLAGS: changing that invalidates the whole
  # build cache and forces a full workspace rebuild on the next command.
  env.CARGO_BUILD_JOBS = "4";

  apple.sdk = null;
  dotenv.disableHint = true;

  # Proofs run through `devenv --profile kani shell`, matching Pina's setup.
  # The arithmetic harnesses pin `cvc5` because it decides the wide integer
  # multiplication and division formulas that the 128-bit pods generate, while
  # `z3` has to be timed out on the signed 128-bit division proof. Both solvers
  # ship here so a harness can switch between them during investigation, and so
  # the same proofs run locally instead of only in CI.
  profiles.kani.module.packages = [
    kani
    pkgs.cvc5
    pkgs.z3
  ]
  ++ kaniGccShim;

  scripts = {
    "build:all" = {
      exec = ''
        set -euo pipefail
        cargo build --workspace --all-features --locked
      '';
      description = "Build every crate with all features enabled.";
      binary = "bash";
    };
    "build:no-default" = {
      exec = ''
        set -euo pipefail
        cargo check --manifest-path pinapod/Cargo.toml --no-default-features --locked
        cargo check --manifest-path pinapod/Cargo.toml --no-default-features --features fixed --locked
        # A host check cannot prove the no_std promise: std is always available
        # to the host target. `cargo check --target` for a bare-metal target
        # resolves std away, so any accidental std use (including one smuggled
        # in through an optional dependency's default features) fails here.
        # Check every optional feature, not only `fixed`, so a new dependency
        # cannot quietly break the promise either. `check` needs no linker for
        # this target, so the job stays a pure metadata/borrowck compile.
        cargo check \
          --manifest-path pinapod/Cargo.toml \
          --no-default-features \
          --features fixed,floats,solana-address,solana-program-error,wincode \
          --target thumbv7em-none-eabihf \
          --locked
      '';
      description = "Verify the no_std PinaPod core with no features, with fixed-point support, and every optional feature against a bare-metal target.";
      binary = "bash";
    };
    "test:all" = {
      exec = ''
        set -euo pipefail
        test:publish
        cargo test --workspace --all-features --locked
      '';
      description = "Run the complete workspace test suite.";
      binary = "bash";
    };
    "test:publish" = {
      exec = ''
        set -euo pipefail
        bun test "$DEVENV_ROOT/scripts/publish-provenance.test.ts"
      '';
      description = "Verify release provenance gates against real git history and MonoChange records.";
      binary = "bash";
    };
    "test:miri" = {
      exec = ''
        set -euo pipefail
        cargo miri test --manifest-path pinapod/Cargo.toml --all-features --locked
      '';
      description = "Run PinaPod's zero-copy regression suite under Miri.";
      binary = "bash";
    };
    # Proof shards mirror the CI matrix so a local run reproduces a red job.
    # The harness argument is required: a missing one would otherwise run the
    # entire proof suite under a single name.
    "test:kani" = {
      exec = ''
        set -euo pipefail
        if [[ -z "''${1:-}" ]]; then
          echo "usage: test:kani <harness>" >&2
          echo "       run test:kani:all to verify every proof" >&2
          exit 2
        fi
        cargo-kani \
          --manifest-path pinapod/Cargo.toml \
          --features kani,floats \
          --output-format terse \
          --harness "$1"
      '';
      description = "Run one Kani proof shard by harness name, e.g. test:kani u128_proofs.";
      binary = "bash";
    };
    # Times every harness in a shard so the slow proofs are visible instead of
    # only surfacing as a CI timeout.
    "test:kani:time" = {
      exec = ''
        set -euo pipefail
        if [[ -z "''${1:-}" ]]; then
          echo "usage: test:kani:time <shard>  (shard is the harness module, e.g. u128_proofs)" >&2
          exit 2
        fi
        for harness in $(grep -rhoE 'fn [a-z0-9_]+\(' pinapod/src/pod/numeric.rs | sed -E 's/fn ([a-z0-9_]+)\(/\1/' | sort -u); do
          start=$(date +%s)
          cargo-kani \
            --manifest-path pinapod/Cargo.toml \
            --features kani,floats \
            --output-format terse \
            --harness "$1::$harness" >/dev/null 2>&1 || true
          printf '%s %ss\n' "$harness" "$(( $(date +%s) - start ))"
        done
      '';
      description = "Time each proof harness in a shard to locate slow proofs.";
      binary = "bash";
    };
    "test:kani:all" = {
      exec = ''
        set -euo pipefail
        cargo-kani \
          --manifest-path pinapod/Cargo.toml \
          --features kani,floats \
          --output-format terse
      '';
      description = "Run every Kani proof in the workspace.";
      binary = "bash";
    };
    "coverage:all" = {
      exec = ''
        set -euo pipefail
        mkdir -p "$DEVENV_ROOT/target/coverage"
        cargo llvm-cov \
          --workspace \
          --all-features \
          --locked \
          --lcov \
          --output-path "$DEVENV_ROOT/target/coverage/lcov.info"
      '';
      description = "Generate workspace LCOV coverage.";
      binary = "bash";
    };
    "bench:compare" = {
      exec = ''
        set -euo pipefail
        if [[ "$(uname -s)" == "Darwin" ]]; then
          task_sdk="$(xcrun --show-sdk-path)"
          export SDKROOT="$task_sdk"
          export RUSTFLAGS="''${RUSTFLAGS:-} -C link-arg=-isysroot -C link-arg=$task_sdk"
        fi
        cargo bench --manifest-path pinapod/Cargo.toml --bench api_comparison --locked
      '';
      description = "Compare current PinaPod with pinned PinaPod v0.1 and upstream ZeroPod fixed and compact APIs.";
      binary = "bash";
    };
    "bench:compare:baseline" = {
      exec = ''
        set -euo pipefail
        if [[ "$(uname -s)" == "Darwin" ]]; then
          task_sdk="$(xcrun --show-sdk-path)"
          export SDKROOT="$task_sdk"
          export RUSTFLAGS="''${RUSTFLAGS:-} -C link-arg=-isysroot -C link-arg=$task_sdk"
        fi
        cargo bench --manifest-path pinapod/Cargo.toml --bench api_comparison --locked -- --save-baseline pinapod-v2-before
      '';
      description = "Save the current three-way measurements as the v0.2 migration baseline.";
      binary = "bash";
    };
    "bench:compare:after" = {
      exec = ''
        set -euo pipefail
        if [[ "$(uname -s)" == "Darwin" ]]; then
          task_sdk="$(xcrun --show-sdk-path)"
          export SDKROOT="$task_sdk"
          export RUSTFLAGS="''${RUSTFLAGS:-} -C link-arg=-isysroot -C link-arg=$task_sdk"
        fi
        cargo bench --manifest-path pinapod/Cargo.toml --bench api_comparison --locked -- --baseline pinapod-v2-before
      '';
      description = "Compare the current measurements with the saved v0.2 migration baseline.";
      binary = "bash";
    };
    # Instruction counts are a property of the binary, so the gate is exact on
    # any Linux host and needs no timing discipline. Valgrind is Linux-only,
    # which is why these tasks exist here rather than in a cross-platform
    # script: every non-Linux host can still compile the bench target.
    "bench:iai" = {
      exec = ''
        set -euo pipefail
        cargo bench --manifest-path pinapod/Cargo.toml --bench iai_hot_paths --locked -- --save-summary=json
        deno run --allow-read --allow-write=pinapod/benches scripts/check-iai-baseline.ts
      '';
      description = "Run the instruction-count gate against the committed baseline. Requires Linux with Valgrind.";
      binary = "bash";
    };
    "bench:iai:update" = {
      exec = ''
        set -euo pipefail
        cargo bench --manifest-path pinapod/Cargo.toml --bench iai_hot_paths --locked -- --save-summary=json
        deno run --allow-read --allow-write=pinapod/benches scripts/check-iai-baseline.ts --update
      '';
      description = "Regenerate the committed instruction-count baseline from the current checkout. Requires Linux with Valgrind.";
      binary = "bash";
    };
    "fix:format" = {
      exec = ''
        set -euo pipefail
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
        docs:sync
      '';
      description = "Format every supported source and configuration file, then re-sync mdt-managed docs.";
      binary = "bash";
    };
    "fix:clippy" = {
      exec = ''
        set -euo pipefail
        cargo clippy --fix --allow-dirty --allow-staged --workspace --all-targets --all-features --locked
      '';
      description = "Apply safe Clippy fixes across every workspace target.";
      binary = "bash";
    };
    "fix:all" = {
      exec = ''
        set -euo pipefail
        fix:clippy
        # `fix:format` runs dprint *before* its own docs:sync, so a block mdt
        # rewrites there would land after formatting and never be formatted
        # itself. Syncing here puts the mdt update ahead of the formatting pass,
        # so the final `fix:format` formats the synchronized blocks it produces.
        docs:sync
        fix:format
      '';
      description = "Fix everything: apply Clippy fixes on every target, sync mdt docs, then format.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -euo pipefail
        cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
      '';
      description = "Run the shared Pina Rust and Clippy lint policy on every target, failing on any warning.";
      binary = "bash";
    };
    "lint:format" = {
      exec = ''
        set -euo pipefail
        dprint check --config "$DEVENV_ROOT/dprint.json"
      '';
      description = "Check formatting without changing files.";
      binary = "bash";
    };
    "lint:monochange" = {
      exec = ''
        set -euo pipefail
        ${custom.monochange}/bin/monochange check
      '';
      description = "Validate release metadata and Cargo manifests.";
      binary = "bash";
    };
    "docs:build" = {
      exec = ''
        set -euo pipefail
        mdbook build "$DEVENV_ROOT/docs"
      '';
      description = "Build the mdBook documentation for GitHub Pages.";
      binary = "bash";
    };
    "docs:api" = {
      exec = ''
        set -euo pipefail
        RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
      '';
      description = "Build public API documentation and reject warnings.";
      binary = "bash";
    };
    "docs:sync" = {
      exec = ''
        set -euo pipefail
        mdt update --path "$DEVENV_ROOT"
      '';
      description = "Sync reusable documentation blocks with mdt.";
      binary = "bash";
    };
    "docs:check" = {
      exec = ''
        set -euo pipefail
        mdt check --path ${lib.escapeShellArg currentDir}
      '';
      description = "Check that reusable documentation blocks are synchronized.";
      binary = "bash";
    };
    "verify:docs" = {
      exec = ''
        set -euo pipefail
        docs:check
        [ -f "$DEVENV_ROOT/docs/book.toml" ]
        [ -f "$DEVENV_ROOT/docs/src/SUMMARY.md" ]
        mdbook build "$DEVENV_ROOT/docs" -d "$DEVENV_ROOT/target/mdbook"
        docs:api
      '';
      description = "Verify the mdBook structure, reusable blocks, and public API documentation.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -euo pipefail
        lint:clippy
        lint:format
        lint:monochange
        verify:docs
      '';
      description = "Run formatting, Rust, manifest, release, and documentation checks.";
      binary = "bash";
    };
    "security:deny" = {
      exec = ''
        set -euo pipefail
        cargo-deny check advisories bans licenses sources
      '';
      description = "Check dependency advisories, licenses, sources, and bans.";
      binary = "bash";
    };
    "security:audit" = {
      exec = ''
        set -euo pipefail
        cargo-audit audit --deny yanked --file "$DEVENV_ROOT/Cargo.lock"
        # The fuzz suite is a standalone workspace and part of the security
        # boundary, so its registry dependencies are audited too.
        cargo-audit audit --deny yanked --file "$DEVENV_ROOT/fuzz/Cargo.lock"
      '';
      description = "Audit the locked dependency graphs, including the fuzz suite, against RustSec.";
      binary = "bash";
    };
    "security:secrets" = {
      exec = ''
        set -euo pipefail
        gitleaks detect --redact --no-banner
      '';
      description = "Scan the commit history for committed secrets.";
      binary = "bash";
    };
    "security:zizmor" = {
      exec = ''
        set -euo pipefail
        zizmor --no-online-audits --no-progress ${lib.escapeShellArg "${currentDir}/.github"}
      '';
      description = "Audit GitHub Actions workflows offline.";
      binary = "bash";
    };
    "verify:security" = {
      exec = ''
        set -euo pipefail
        security:deny
        security:audit
        security:secrets
        security:zizmor
      '';
      description = "Run dependency, secret, and workflow security checks.";
      binary = "bash";
    };
  };
}
