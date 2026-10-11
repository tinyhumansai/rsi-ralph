# Lineage ledger

`Lineage` is the small, step-ordered ledger that the TinyBus module serves. It
holds `GradeEntry` values (defined in `crates/tinyrsi-bus/src/lineage/`) and
answers one question: which entry is the current incumbent.

| File | Responsibility |
| --- | --- |
| `mod.rs` | `Lineage`: append validation, head lookup, read accessors |
| `test.rs` | Step ordering, empty-field rejection and head selection |

## Public surface

| Item | Behavior |
| --- | --- |
| `Lineage::new` | Empty ledger |
| `Lineage::append(entry)` | Records `entry` and returns `Ok(true)` when it is valid |
| `Lineage::head()` | Last entry whose verdict is `Accepted`, or `None` |
| `Lineage::entries`, `len`, `is_empty` | Read access in step order |
| `tinyrsi::append`, `tinyrsi::head` | Crate-root wrappers taking and returning bus payloads |

An entry is valid when:

- `entry.step` equals the ledger's current length (the first entry is step 0,
  and steps never skip or repeat); otherwise `Error::StepOutOfOrder`;
- `entry.snapshot` and `entry.rationale` are non-empty after trimming;
  otherwise `Error::EmptyField`.

Rejected entries are recorded too. A rejected candidate takes a step, but it
never becomes the head, so the head is always the most recent acceptance.

```rust
use tinyrsi::{Lineage, GradeEntry};
use tinyrsi_bus::Verdict;

let mut lineage = Lineage::new();
lineage.append(GradeEntry::new(0, "sha256:a", 0.70, Verdict::Accepted, "baseline"))?;
lineage.append(GradeEntry::new(1, "sha256:b", 0.65, Verdict::Rejected, "no gain"))?;
assert_eq!(lineage.head().map(|e| e.step), Some(0));
# Ok::<(), tinyrsi::Error>(())
```

## Relation to `Archive`

The crate has two ledgers, and they are not linked:

- `Lineage` (this module) is what a TinyBus host sees. It stores only a step,
  a snapshot address string, a grade, a verdict and a rationale. Nothing in
  it is checked against real packages.
- `Archive` (`src/archive/`) is what the improvement loop uses. It stores
  exact `tinyhivemind_lang` records and packages, content identities,
  proposals, paired evaluations and approvals, and it validates ancestry.

`run_proposal_h` writes only to `Archive`. Nothing in the crate copies archive
outcomes into a `Lineage`; a host that wants bus clients to see loop results
must call `Append` itself.

## Operational constraints

- The served ledger is in memory. `tinybus_module::setup` creates a fresh
  `Lineage` behind a `Mutex` when the module loads, so a host restart or a
  module reload starts again at step 0. Durable history belongs in `Archive`.
- One ledger per loaded module instance. There is no namespacing by hive or
  run.
- `grade` is an opaque `f64`; the ledger neither validates its range nor
  compares grades. The writer owns the scale.
