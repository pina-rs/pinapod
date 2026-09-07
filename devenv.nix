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
in
{
  packages = with pkgs; [
    cargo-audit
    cargo-deny
    cargo-llvm-cov
    dprint
    gh
    git
    gitleaks
    mdbook
    custom.monochange
    nixfmt-rfc-style
    rustup
    shfmt
    zizmor
  ];

  env.CARGO_TERM_COLOR = "always";

  apple.sdk = null;
  dotenv.disableHint = true;

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
      '';
      description = "Verify the no_std PinaPod core with no features and with fixed-point support.";
      binary = "bash";
    };
    "test:all" = {
      exec = ''
        set -euo pipefail
        cargo test --workspace --all-features --locked
      '';
      description = "Run the complete workspace test suite.";
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
    "fix:format" = {
      exec = ''
        set -euo pipefail
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
      '';
      description = "Format every supported source and configuration file.";
      binary = "bash";
    };
    "fix:clippy" = {
      exec = ''
        set -euo pipefail
        cargo clippy --fix --allow-dirty --allow-staged --workspace --all-features --locked
      '';
      description = "Apply safe Clippy fixes across the workspace.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -euo pipefail
        cargo clippy --workspace --all-features --locked -- -D warnings
      '';
      description = "Run the shared Pina Rust and Clippy lint policy.";
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
    "verify:docs" = {
      exec = ''
        set -euo pipefail
        [ -f "$DEVENV_ROOT/docs/book.toml" ]
        [ -f "$DEVENV_ROOT/docs/src/SUMMARY.md" ]
        mdbook build "$DEVENV_ROOT/docs" -d "$DEVENV_ROOT/target/mdbook"
        docs:api
      '';
      description = "Verify the mdBook structure and public API documentation.";
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
        cargo-deny check bans licenses sources
      '';
      description = "Check dependency licenses, sources, and bans.";
      binary = "bash";
    };
    "security:audit" = {
      exec = ''
        set -euo pipefail
        cargo-audit audit --deny yanked --file "$DEVENV_ROOT/Cargo.lock"
      '';
      description = "Audit the locked dependency graph against RustSec.";
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
        security:zizmor
      '';
      description = "Run dependency and workflow security checks.";
      binary = "bash";
    };
  };
}
