# Releasing Pinapod

Pinapod uses monochange for changesets, release pull requests, version updates, tags, crates.io publication, and GitHub releases. Run every command through the repository's devenv shell.

```sh
devenv shell
monochange run change --package pinapod --bump patch --reason "Describe the user-facing change"
monochange step validate
monochange step prepare-release --dry-run --format json
```

Merging a feature pull request adds its changeset to `main`. The `release-pr.yml` workflow creates or refreshes—but never auto-merges—the `chore(release): prepare release` pull request. Merging that release pull request causes monochange to create the version tag and draft GitHub release, then dispatches `publish.yml` to publish both crates in dependency order and make the GitHub release public.

## First release bootstrap

Trusted publishing cannot create a new crates.io package. Before merging the first release pull request, a registry owner must reserve both package names and configure crates.io trusted publishers for repository `pina-rs/pinapod`, workflow `publish.yml`, and environment `publisher`.

```sh
monochange step placeholder-publish --dry-run --package pinapod_derive
monochange step placeholder-publish --dry-run --package pinapod
```

The real placeholder publication is a maintainer-owned, one-time operation; the normal release workflow uses GitHub OIDC and no long-lived crates.io token.
