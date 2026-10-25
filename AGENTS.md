# Repository Guidelines

This file is the single source of truth for how humans and coding agents work
in this repository. `CLAUDE.md` is a symlink to this file, so every agent reads
the same instructions.

RSI Ralph is a host-driven improvement loop for tinyhivemind hives and plain
scripts, shipped as a Rust library. It runs recursive Ralph loops: OpenHuman
re-edits a hive's language files until a goal is properly met. Start with
[`docs/architecture.md`](docs/architecture.md) for the shape of one epoch and
[`docs/glossary.md`](docs/glossary.md) for the vocabulary used throughout the
code and docs.

## Project Structure

This is a Rust 2024 cargo workspace rooted at a virtual `Cargo.toml`. Every
crate lives under `crates/`, one directory per package, each directory named for
the package it holds. There is no root package: the library hosts depend on is
`crates/rsi-ralph`, the same as any other member. The crates are split by loop
phase; see [`docs/specs/crate-split.md`](docs/specs/crate-split.md).

```text
Cargo.toml              # virtual workspace: members, [workspace.package],
                        # [workspace.dependencies], [workspace.lints]
ROADMAP.md              # shipped / next / out of scope
crates/
├── rsi-ralph-types/      # proposal and candidate data shared by every crate
├── rsi-ralph-model/      # ModelPort, OpenRouterModel, metering, block_on
├── rsi-ralph-trace/      # `events.jsonl` loop trace and its `tracing` mirror
├── rsi-ralph-langfuse/   # Langfuse export of loop traces via openhuman-embed
├── rsi-ralph-gateway/    # spend-capped loopback proxy to OpenRouter, one per run
├── rsi-ralph-score/      # Scorer port, judges, decide, paired stats, experiments
├── rsi-ralph-verify/     # host checkers, blueprints, Lake, process runner
├── rsi-ralph-evolve/     # Evolver port, script supervisor, retreat and its memory
├── rsi-ralph-cli/        # the `rsi-ralph` binary: file-defined loops, run store, UI
├── rsi-ralph-host/       # hive runtime: coordinators, suites, epochs, deployment
│   ├── src/            # host/, suites/, epoch_driver/, evidence_journal/
│   └── examples/       # complete_openrouter, observe_epochs, private benchmarks
├── rsi-ralph-swarm/      # a hive package run as OpenHuman agents editing candidates
└── rsi-ralph/            # the loop library, re-exporting the crates above
    ├── src/
    │   ├── lib.rs            # crate docs + the entire public surface
    │   ├── error/            # crate-wide `Error` and `Result<T>`
    │   ├── lineage/          # small in-memory ledger of graded candidates
    │   ├── archive/          # durable selection history (records, proposals, evidence)
    │   ├── observation/      # read-only coordinator snapshots and dossiers
    │   ├── orchestrator/     # proposal funnel: run_proposal, run_proposal_h
    │   ├── memory/           # immutable hive-memory snapshot store
    │   └── script_loop/      # improvement loop over a plain script
    ├── tests/          # integration tests against the public API only
    └── examples/       # runnable, compiled-in-CI usage examples
examples/               # `rsi-ralph-examples`: math_problem, upgrade_file, graph_coloring,
                        # hive_goal
loops/                  # example loop folders for the `rsi-ralph` binary
vendor/tinyhivemind/    # pinned hive language, coordinator and storage
vendor/tinybackrooms/   # pinned creative-retreat engine
vendor/openhuman/       # pinned OpenHuman, for its embed Langfuse client
apps/rsi-viz/           # read-only Next.js loop visualizer
docs/                   # see docs/README.md for what lives where
```

`crates/rsi-ralph-host` holds what needs a live coordinator (host runtime,
private suites, durable epochs, evidence journal) and adds SQLite coordinator
storage and Tokio; it replaced the former `host` Cargo feature. Each module
directory follows the
`mod.rs` / `types.rs` / `test.rs` layout below, and most carry a `README.md`.

### Adding a crate

Add a crate by creating `crates/<name>/`; `members = ["crates/*",
"examples"]` picks it up by existing. Inherit `version`, `edition`,
`rust-version`, `license`, and `repository` from `[workspace.package]`, take
shared dependencies from `[workspace.dependencies]`, and opt into the shared
lint set with:

```toml
[lints]
workspace = true
```

Each feature area belongs in a focused module directory under a crate's `src/`.
A module root explains the module, wires its pieces together, and exposes the
smallest useful API. Move substantial type definitions into `types.rs` and put
module-local unit tests in a dedicated `test.rs`, wired from the bottom of the
module root with:

```rust
#[cfg(test)]
mod test;
```

Do not accumulate inline `mod tests` blocks in implementation files, and do not
let a general-purpose `utils.rs` or `helpers.rs` grow — those are a symptom of a
missing module. Prefer many small modules that each do one thing well over few
broad ones.

Keep public exports centralized in each crate's `src/lib.rs` so downstream users
have one predictable surface. Put shared error variants in
`crates/rsi-ralph/src/error/mod.rs` and return the crate-wide `Result<T>` from
fallible public APIs.

## Build And Test

Run every command from the repository root. These four are the contract; CI
runs exactly them, so a green local run should mean a green CI run.

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

Supporting commands:

- `cargo fmt` — format before committing. Leave off `--all`: it also
  formats the vendored path dependencies, which are never edited here.
- `cargo test <filter>` — run a focused subset while iterating.
- `cargo test -p rsi-ralph` — run one crate's suite.
- `cargo run -p rsi-ralph --example basic` — run the minimal lineage example.
- `cargo run -p rsi-ralph-examples --example math_problem` (or `upgrade_file`,
  `graph_coloring`, `hive_goal`) — the offline end-to-end examples in `examples/`; each writes a
  trace under `target/rsi-examples/` and fails if it misses its goal.
- `cargo run -p rsi-ralph-cli -- check loops/graph-coloring` (or `run`, `runs`,
  `show`, `serve`) — the `rsi-ralph` binary over a loop folder; see
  `docs/guides/rsi-cli.md`.
- `cargo run -p rsi-ralph-examples --example benchmarks -- --repeats 2` — benchmark × model × repeat matrix of the script loop; writes `summary.md` under `target/rsi-benchmarks/` (see `docs/specs/loop-benchmarks.md`).
- `cargo run -p rsi-ralph-host --example observe_epochs` — offline
  two-epoch observation demo; writes receipts under `target/rsi-observe-epochs/`.
- `cargo run -p rsi-ralph-host --example complete_openrouter` — the
  full live loop; does nothing unless `TINYRSI_LIVE_OPENROUTER=1` and needs
  `OPENROUTER_API_KEY` and `TINYRSI_LEAN` (see `docs/rsi/complete-live.md`).
- `cargo doc --no-deps --all-features` — build the rustdoc CI also builds with
  `RUSTDOCFLAGS="-D warnings"`.
- `cargo test --doc` — run doctests alone when editing documentation examples.

Other examples: `live_openrouter` (arithmetic smoke, opt-in like the above),
`private_benchmark`, `proposal_metrics` and `verify_lean_slice` (all in
`rsi-ralph-host`; the last builds private Lean proofs with a pinned Lake), and
`script_loop` (offline curve-fitting run of the plain-script loop).

Never skip, ignore, or delete a failing test to make a command pass. Fix the
root cause, or stop and report the blocker.

## Coding Style

Use standard `rustfmt` output and Rust 2024 idioms. Do not hand-format around
`rustfmt`, and do not add `#[rustfmt::skip]` without a comment explaining why.

- `snake_case` for modules, files, functions, methods, fields, and locals.
- `PascalCase` for types, traits, and enum variants; `SCREAMING_SNAKE_CASE` for
  constants and statics.
- Name things for what they are, not for their layer: `RetryPolicy`, not
  `RetryHelper`.
- Prefer small, typed APIs over stringly-typed ones. Accept `&str` and generic
  `impl Into<String>` at boundaries; return owned, concrete types.
- Keep the public surface minimal: default to private, and export deliberately
  from `src/lib.rs`.
- `unsafe` is forbidden workspace-wide by `[workspace.lints]` in the root
  `Cargo.toml`. If a project genuinely needs it, relax the lint in its own
  commit and document every invariant with a `// SAFETY:` comment.

### Errors

- One crate-wide `Error` enum per crate, in `src/error/mod.rs`, built with
  `thiserror`.
- Fallible public functions return `Result<T>`, the crate alias.
- Add a specific variant instead of stuffing context into a string; error
  messages are lowercase, without trailing punctuation.
- Do not `unwrap()`, `expect()`, or `panic!` in library code paths. They are
  fine in tests, examples, and genuinely unreachable states — where `expect`
  must carry a message explaining the invariant.
- Document a `# Errors` section on every public fallible function and a
  `# Panics` section on anything that can panic.

### Dependencies

Adding a dependency is a design decision. Before adding one, check whether the
standard library or an existing dependency already covers the need. When you do
add one:

- pin a caret range (`serde = "1"`), not an exact version;
- enable only the features you need, with `default-features = false` when that
  meaningfully trims the tree;
- gate anything optional behind a Cargo feature, documented in `Cargo.toml`;
- declare it once in the root `[workspace.dependencies]` when more than one
  crate needs it, and take it with `{ workspace = true }`;
- leave a comment above the entry explaining *why* the crate is needed and what
  uses it — see the existing entries for the expected tone;
- prefer well-maintained crates with a compatible license.

Keep `Cargo.lock` committed; this workspace ships a single lockfile so CI and
releases are reproducible.

### Vendored dependencies

tinyhivemind, tinybackrooms and OpenHuman are registered as the
`vendor/tinyhivemind`, `vendor/tinybackrooms` and `vendor/openhuman` git
submodules and pinned by their gitlinks. They supply the hive language,
coordinator and storage, the creative-retreat engine, and the Langfuse client
`rsi-ralph-langfuse` exports through. Initialize them after cloning with:

```sh
git submodule update --init --recursive
```

Do not edit vendored code from the parent repository. Make changes in the
vendored project's own repository, push them there, then update this
repository's gitlink in a separate commit. Keep the exact path dependencies and
minimal features unless a new capability requires more. The root `[patch]`
tables mirror `vendor/openhuman/Cargo.toml`; update them in the same commit as
that gitlink.

## Testing

- Module-local unit tests live in `crates/<crate>/src/<feature>/test.rs` and may
  touch private items.
- Integration tests live in `crates/<crate>/tests/` and exercise only the public
  API — they are the regression suite for the crate's contract.
- Persisted and wire types pin their serde representation in a unit test.
  That representation is the stored form: a writer and a reader that disagree
  about a field name fail at runtime with a decode error.
- Use descriptive, behavioral test names: `rejects_an_empty_name`, not
  `test_greet_2`.
- Cover the failure paths, not just the happy path. Every new error variant
  needs a test that produces it.
- For async behavior, standardize on one runtime (`tokio` as a dev-dependency
  for tests) rather than mixing runtimes.
- Tests must be deterministic and independent of network, wall-clock time, and
  execution order. Gate any live/network test behind a feature or an env var and
  name it `live_*` so it is easy to exclude.
- Maintain at least 90% line coverage in every source file. Add or update tests
  with every behavior change, and note any deliberately untested edge case in
  the pull request description.

Write the test first when fixing a bug: a failing test that reproduces the
report, then the fix that turns it green.

## Documentation

Write documentation for the reader who has never seen the code.

- Every public item gets a rustdoc comment. `missing_docs` is a warning that CI
  treats as an error.
- Start every `mod.rs` and `test.rs` with a concise module-level `//!`
  description.
- Each crate's `src/lib.rs` carries its crate-level overview: what the crate
  does, the primary entry points, and a short runnable example. It should also
  say what the crate deliberately does *not* hold, and why.
- Prefer concrete examples over vague description. Doc examples are compiled and
  run by `cargo test`, so they cannot drift.
- Complex modules must include a module-level `README.md` covering their design,
  public surface, and important operational constraints.
- Keep `README.md`, `docs/`, and module docs aligned with code changes in the
  same commit that changes behavior.
- Write accepted behavior and constraints in `docs/specs/` before creating a
  linked, implementation-ordered plan in `docs/plans/`. Specs define what and
  why; plans define how and in what sequence.
- Keep every Markdown file, including this one, at 500 lines or fewer. When a
  topic outgrows that, split it into focused files and link them from the
  nearest `README.md`.

## Git Workflow

- Never commit directly to `main`. Branch first, one branch per logical change.
- Do feature work in a git worktree so the main checkout stays clean.
- Commit subjects are concise and imperative: `Add retry policy to the client`.
  Keep the subject specific to the change and under ~72 characters.
- Make small, focused commits. Each commit should cover one logical change,
  build independently, and avoid mixing formatting, refactors, and behavior
  changes unless they are inseparable.
- Never commit secrets. `.env` is git-ignored; document new variables in
  `.env.example` with placeholder values.
- Never force-push a shared branch, rewrite published history, or bypass hooks
  with `--no-verify`.

## Pull Requests

Open pull requests ready for review, not as drafts, unless the work genuinely
must not merge yet. A pull request should:

- summarize what changed and why, in a few sentences;
- call out public API or behavior changes explicitly, or state "None";
- list the validation commands actually run, with their outcome;
- link the related issue;
- include updated tests, docs, and examples in the same change.

The template in `.github/PULL_REQUEST_TEMPLATE.md` encodes this checklist.
Address review feedback by fixing it, and reply on each thread describing what
changed. Do not resolve a thread whose feedback you have not addressed or
explicitly declined with a reason.

## Releases

Releases run from `.github/workflows/release.yml` via a manual
`workflow_dispatch` with a `patch` / `minor` / `major` bump; `current` resumes
an interrupted release after its version commit and tag exist. The workflow
re-runs the full validation suite, computes the next version, updates
the root `[workspace.package]` version and `Cargo.lock`, commits and tags
`vX.Y.Z`, pushes, and creates an immutable GitHub release with generated
notes, then publishes the `rsi-ralph` binary as the `ghcr.io/<owner>/rsi-ralph`
container image from the `Dockerfile`. A release is a tagged source version;
there are no other binary assets.

Consequently:

- Do not hand-edit the `version` field in the root `[workspace.package]`; the
  release workflow owns it. Every member inherits it with
  `version.workspace = true`, so the whole workspace releases as one version.
- Follow semantic versioning. Any change to the public surface that is not
  purely additive is a breaking change and needs a major bump (pre-1.0: a minor
  bump).
- `main` should always be green, so any commit on it can be released.

## Agent Working Agreement

For automated contributors specifically:

1. **Read before writing.** Inspect the surrounding module and match its
   conventions, comment density, and idiom rather than importing a house style.
2. **Verify, do not assume.** Run the four contract commands and read their
   output before reporting a task complete. Report failures with the output;
   never claim a check passed that you did not run.
3. **Stay in scope.** Implement what was asked. Do not opportunistically
   refactor, reformat, upgrade dependencies, or "fix" unrelated code — raise it
   instead.
4. **No placeholders in delivered code.** No `todo!()`, no stubbed functions, no
   commented-out alternatives left behind. If something cannot be finished, say
   so explicitly.
5. **Do not weaken the guardrails.** Never add blanket `#[allow(...)]`, relax a
   lint, mark a test `#[ignore]`, or loosen CI to get a green run. Fix the
   cause.
6. **Secrets stay out.** Never read, echo, or commit `.env` contents, tokens, or
   credentials, and never paste them into a pull request or issue.
7. **Ask only when blocked.** Make routine judgment calls yourself; escalate
   only irreversible decisions or genuine forks with no clear default.
