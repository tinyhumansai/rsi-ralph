# Loop benchmarks implementation

Contract: [benchmark harness for the script loop](../specs/loop-benchmarks.md).

One change, in test-first order. Each step ends green on the four contract
commands. The examples package is outside the `crates/` coverage gate, but
the new library modules still aim for 90% line coverage.

## Assumptions

- `tinyrsi-examples` keeps its existing error style (`Box<dyn Error>`), as
  every function in its library already does; the runner adds no error enum.
- No new dependency: `serde` (derive) is already a workspace dependency and
  is added to the examples package; `std::thread::scope` covers `--jobs`.
- Python 3 is available wherever the tests run, as the existing examples and
  `script_loop` tests already assume.

## 1. Documentation

- `docs/specs/loop-benchmarks.md`, this plan, and both indexes.

## 2. Shared benchmark definitions

1. Move the curve-fit brief to `crates/tinyrsi/examples/script_loop/brief.txt`
   and read it with `include_str!` in `main.rs`; output is unchanged.
2. `examples/src/benchmarks/`:
   - `mod.rs`: the `Benchmark` enum (`ALL`, `name`, `from_name`,
     `offline_supervisor`, `offline_judge`, `run`) and `Models`.
   - one file per benchmark (`curve_fit.rs`, `graph_coloring.rs`,
     `math_problem.rs`, `upgrade_file.rs`) holding its `include_str!`
     fixtures, brief, canned replies, grader, scorer and policy, moved from
     the example `main.rs` files verbatim.
   - `test.rs`: names round-trip, unknown names fail, every benchmark's canned
     queue is non-empty, metric-only benchmarks have no judge, a judged
     benchmark without a judge fails with `judge required`, and
     `math_problem` runs offline to its goal.
3. Rewrite `examples/{math_problem,upgrade_file,graph_coloring}/main.rs` to
   call `Benchmark::run`, keeping their printed line and goal assertion.
4. `live_model(id)` in `examples/src/lib.rs`; `live_or` uses it.

## 3. Measurement

`examples/src/harness/`:

1. `types.rs`: `RunRecord`, `SummaryRow`, `Summary` (serde).
2. Tests first in `test.rs`:
   - a ledger with a baseline, an accepted and promoted candidate, a
     rejection, a duplicate of that rejection, an invalid row and a
     same-hypothesis resubmission yields the documented counts and
     `repetition_share`;
   - a duplicate of the incumbent is not a repeat; reworded hypotheses
     normalize equal; an empty run has no share;
   - a missing ledger gives an `error` record;
   - aggregation: goal rate, medians (odd and even), means skipping `null`;
   - Markdown renders `–` for `null` and `k/N`;
   - `summary.json` field names are pinned.
3. `mod.rs`: `record`, `repetition`, `normalize_hypothesis`, `aggregate`,
   `render_markdown`, `model_label`.

## 4. Command line and runner

1. `harness/options.rs`: `Options::parse` (tests first: defaults, every
   flag, unknown flag, unknown benchmark, missing value, zero repeats).
2. `harness/runner.rs`: `plan` (matrix order and run directories) and
   `execute` (sequential or `--jobs` scoped threads, records sorted back
   into matrix order, `summary.json` and `summary.md` written). A test runs
   a one-run offline matrix of `math_problem`.
3. `examples/benchmarks/main.rs`: parse, choose models (offline label or
   `live_model`), execute, print the table, fail offline if a goal is missed.
4. Register the example in `examples/Cargo.toml`; add it to the CI step that
   runs the offline examples with `--repeats 1`.

## 5. Docs

- `examples/README.md`: the runner, its flags and output.
- `AGENTS.md`: one supporting-command line.
- `docs/rsi/script-loop-roadmap.md`: item 6 implemented, linking the spec.

## Verification

```sh
cargo +1.99 fmt -- --check
cargo +1.99 clippy --all-targets --all-features -- -D warnings
cargo +1.99 build --all-targets --all-features
cargo +1.99 test --all-features
RUSTDOCFLAGS="-D warnings" cargo +1.99 doc --no-deps --all-features
cargo +1.99 run -p tinyrsi-examples --example benchmarks -- --repeats 2
cargo +1.99 run -p tinyrsi-examples --example math_problem   # and the other three
```

## Checklist

- [x] Spec and plan
- [x] Shared benchmark definitions
- [x] Measurement
- [x] Command line and runner
- [x] Docs
