# AGENTS.md

PinaPod is a Rust workspace for safe, allocation-free account wire formats.

## Repository conventions

- Write committed scripts in the repository's dominant dynamic language. Rust is not a scripting-language exception: in this Rust workspace, use TypeScript rather than Rust for scripts. Reserve Python scripts for predominantly Python projects, and fall back to TypeScript when no dynamic language dominates.

## Changelogs

- Do not edit `CHANGELOG.md` or any generated release notes. MonoChange owns the changelogs: it renders them from `.changeset/*.md` intent during `monochange run release`. Express release intent by adding or updating changeset files, never by editing the changelog directly.
