# Eden System Development Guide for Agents

## Scope

- These rules apply repository-wide. Read any closer `AGENTS.md` or `CLAUDE.md`; closer rules add
  to or override this file.
- Use `README.md` as the product specification. Ask before coding when required behavior is still 
  ambiguous.

## System boundaries

- Eden v3 bridges one Discord guild and its Minecraft server.
- Never hardcode community-specific names in user-facing behavior. Read the configured community
  alias; community names are allowed only as configuration defaults.
- Never call the Minecraft server directly through RCON or another API. EdenMC owns all
  Minecraft-side operations.

## Architecture / Tech Stack

**Backend**:
- [Rust Programming Language](https://rust-lang.org) (2024 edition)
- [PostgreSQL](https://postgresql.org)
- [Kafka](https://kafka.apache.org)

**Frontend**: TBD

## Changes

- State assumptions before editing. Prefer the smallest correct solution and avoid speculative 
  abstractions or error handling for infallible cases.
- Preserve unrelated changes and human-written dead code.
- Do not use deprecated APIs.
- Prefer self-documenting code. Add inline comments only for safety or genuinely non-obvious
  invariants.
- Make retryable or mutating APIs idempotent where applicable, and use PostgreSQL transactions for 
  atomic multi-step database work.

## Rust

- Use safe Rust. The workspace denies `unsafe_code`; any unavoidable exception requires a documented 
  safety invariant and `#[expect(unsafe_code, reason = "...")]` scoped as narrowly as possible.
- With `error-stack`, use `attach(...)` or `attach_opaque(...)`; never use deprecated
  `attach_printable(...)` APIs.
- Suppress lints only with `#[expect(lint, reason = "...")]`; never add `#[allow(...)]` manually.
- Change database structure through migrations, then regenerate `crates/eden-database/src/schema.rs` 
  with `diesel-cli` instead of editing generated schema code by hand.
- In `xtask/src/flags.rs`, edit only the `xflags!` block. Regenerate the section between
  `// generated start` and `// generated end` with:

  ```sh
  env UPDATE_XFLAGS=1 cargo build -p xtask
  ```

## Validation

- Format with `cargo fmt --all` and run Clippy for affected code.
- Run tests through `cargo xtask test [crate]`. This requires `DATABASE_URL`, a reachable PostgreSQL 
  instance, and `pg_dump`; use `--nextest` only when requested or available.
- Add or update tests for changed behavior. If a required check cannot run, report the exact reason.

## Shell

- Prefer `rg`/`rg --files` for searches.
- Avoid pagers, output-truncation commands, and unnecessary pipelines. Use a command's own limiting 
  flags and consume its full output.
