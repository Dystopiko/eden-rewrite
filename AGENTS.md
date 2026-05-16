# Eden System Development Guidelines

## Architecture

**Backend Stack**
- Rust
    - Axum (web server framework)
    - `error-stack` (error handling)
    - `sqlx` (database driver for Postgres)
    - Tokio (async framework)
    - Twilight (for Discord API)
- Postgres

## Repository Structure

This repository is organized into key directories:

- `/crates` - Contains the majority of the source code for Eden system
- `/migrations` - Contains files for database migrations
- `/xtask` - Contians the source code for custom development tasks.

## Code Guidelines

- Always identify which crate a feature belongs to before making it.
- Always write functions that are composable or reusable.
- Always check existing implementations before creating new ones.
- Always USE SPACES in human-readable source and script files, never tabs.
- Always prefer writing safe Rust code.
- Never touch secrets!
- Always ensure new database migrations are reversible to the previous current version.
- Always ask the user to confirm the code if `unsafe` code must be used for performance reasons or
  if a task is not feasible in safe Rust.
- Never directly run the `eden` binary, as it will run the entire Eden system from secret files.
- Never write inline comments unless ABSOLUTELY necessary for clarity.
- Always aim to write self-documentated code.
- Always write `attach(..)` or `attach_opaque(...)` for `error-stack`
  (`attach_printable(...)` is deprecated).
- Never use any deprecated functions in ANY crate.
- Always run `cargo clippy` and fix all reported linting and deprecation issues.
- NEVER MAKE DIRECT RCON OR API CALLS to the Minecraft server from the Eden system.
- Always assume all Minecraft-side logic is handled by the EdenMC mod reaching out to the Eden system.

### Crate-specific Instructions

This repository may have specific instructions for crates. Always review the repository for any
specific instructions related to individual crates (may be found in `AGENTS.md`/`CLAUDE.md` file).

## Common Commands

### Development
- This repository has no binaries other than the `eden` and `xtask` crate.
- Always run custom development tasks by executing `cargo xtask`

### Testing
- Always run the full suite of unit testing by running: `cargo xtask test`
- You may run unit tests for a single crate: `cargo xtask test <crate>`
