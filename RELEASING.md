# Releasing PinaPod

PinaPod uses monochange for changesets, release pull requests, version updates, tags, crates.io publication, and GitHub releases. The `pinapod-workspace` release group contains both `pinapod` and `pinapod-derive`. Its custom version format publishes `pinapod/v*` tags. Run every command through the repository's devenv shell.

```sh
devenv shell
monochange run change --package pinapod --bump patch --reason "Describe the user-facing change"
monochange step validate
monochange step prepare-release --dry-run --format json
```

Use the bump level required by the public API. Version 0.2 uses a minor bump from the pre-1.0 `0.1` line.

## Derive lockstep

The runtime crate pins `pinapod-derive` to its exact released version (`=x.y.z`) because generated code expands against the runtime crate's private contracts. A mixed `pinapod`/`pinapod-derive` pair could emit code the runtime does not match. Both crates release together from the `pinapod-workspace` group, and the release writes the pin through a typed `versioned_files` entry with `prefix = "="` in `monochange.toml` — never by hand.

MonoChange is also the project's semver gate in place of `cargo-semver-checks`: every pull request runs the `changeset-policy` workflow, which enforces changeset coverage and posts a semantic change classification (`detection-level: semantic`) with the proposed bump per package, and the release preview derives compatibility evidence from the semantic diff.

Before merging a feature pull request, run:

```sh
devenv shell test:all
devenv shell test:miri
devenv shell lint:all
devenv shell verify:security
devenv shell bench:compare
```

CI also runs the Kani proof suite. For a performance-sensitive change, record the machine and target with the Criterion result. Compare the current implementation with both pinned historical implementations in the same run.

Merging a feature pull request adds its changeset to `main`. The `release-pr.yml` workflow creates or refreshes the `chore(release): prepare release` pull request, but it never auto-merges that pull request. Merging the release pull request causes monochange to create the version tag and draft GitHub release. It then dispatches `publish.yml` to publish both crates in dependency order and make the GitHub release public.

The documentation workflow builds the mdBook and Rust API docs for changes on `main` and for published `pinapod/v*` releases. It publishes the book to GitHub Pages only after both documentation builds pass.

For a coordinated Pina migration, merge and publish PinaPod first. Then replace Pina's temporary path or git dependency with the released crate, regenerate its clients and docs, and merge the Pina pull request.

## First release bootstrap

Trusted publishing cannot create a new crates.io package. Before merging the first release pull request, a registry owner must reserve both package names and configure crates.io trusted publishers for repository `pina-rs/pinapod`, workflow `publish.yml`, and environment `publisher`.

```sh
monochange step placeholder-publish --dry-run --package pinapod-derive
monochange step placeholder-publish --dry-run --package pinapod
```

The real placeholder publication is a maintainer-owned, one-time operation; the normal release workflow uses GitHub OIDC and no long-lived crates.io token.
