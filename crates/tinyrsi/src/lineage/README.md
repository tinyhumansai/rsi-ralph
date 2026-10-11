# Lineage ledger

`Lineage` is a small, step-ordered, in-memory ledger. It holds `GradeEntry`
values and answers one question: which entry is the current incumbent.

| File | Responsibility |
| --- | --- |
| `mod.rs` | `Lineage`: append validation, head lookup, read accessors |
| `types.rs` | `GradeEntry` and `Verdict` |
| `test.rs` | Step ordering, empty-field rejection, head selection, serde form |

## Public surface

| Item | Behavior |
| --- | --- |
| `Lineage::new` | Empty ledger |
| `Lineage::append(entry)` | Records `entry` and returns `Ok(true)` when it is valid |
| `Lineage::head()` | Last entry whose verdict is `Accepted`, or `None` |
| `Lineage::entries`, `len`, `is_empty` | Read access in step order |

An entry is valid when:

- `entry.step` equals the ledger's current length (the first entry is step 0,
  and steps never skip or repeat); otherwise `Error::StepOutOfOrder`;
- `entry.snapshot` and `entry.rationale` are non-empty after trimming;
  otherwise `Error::EmptyField`.

Rejected entries are recorded too. A rejected candidate takes a step, but it
never becomes the head, so the head is always the most recent acceptance.

```rust
use tinyrsi::{GradeEntry, Lineage, Verdict};

let mut lineage = Lineage::new();
lineage.append(GradeEntry::new(0, "sha256:a", 0.70, Verdict::Accepted, "baseline"))?;
lineage.append(GradeEntry::new(1, "sha256:b", 0.65, Verdict::Rejected, "no gain"))?;
assert_eq!(lineage.head().map(|e| e.step), Some(0));
# Ok::<(), tinyrsi::Error>(())
```

## Relation to `Archive`

The crate has two ledgers, and they are not linked:

- `Lineage` (this module) stores only a step, a snapshot address string, a
  grade, a verdict and a rationale. Nothing in it is checked against real
  packages.
- `Archive` (`src/archive/`) is what the improvement loop uses. It stores
  exact `tinyhivemind_lang` records and packages, content identities,
  proposals, paired evaluations and approvals, and it validates ancestry.

`run_proposal_h` writes only to `Archive`. Nothing in the crate copies archive
outcomes into a `Lineage`.

## Operational constraints

- The ledger is in memory only. Durable history belongs in `Archive`.
- `grade` is an opaque `f64`; the ledger neither validates its range nor
  compares grades. The writer owns the scale.
