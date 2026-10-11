# Strike ledger and cooldown

`Archive::penalties` projects the replayed event history into a
`PenaltyReport`: every `Strike` against a `PatchShape`, in archive order. Like
`proposal_dispositions` it is pure, read-only and never changes a verdict.

A `PatchShape` is a patch's class plus its sorted, deduplicated
`(operation tag, target)` pairs. Targets are the JSON pointer, the document or
prompt path, `path#item` for a context item and `identity#entry` for a
learning operation. Literal before/after values are ignored, so two patches
editing the same targets with different text share a shape.

| Strike | Source |
| --- | --- |
| `Harmful` | A proposal stop with reason `HARMFUL` |
| `Regressed` | A `NO_GAIN` or `NOT_REPLICATED` stop whose last normalized comparison with the same epoch and patch has an upper bound below zero |
| `Revoked` | A `revocation` event written by `Archive::revoke`, for a manual or anchor-regression rollback |

Host failures, inconclusive evidence, approval denials, duplicates, cooldowns
and prefilter rejections are not strikes. The legacy raw funnel has no
normalized evidence, so it never yields `Regressed`.

`PenaltyReport::cooled(shape, epoch, policy)` counts strikes on that shape
from epochs `e <= epoch` with `epoch - e < window_epochs`. A `Harmful` or
`Revoked` strike cools the shape alone; otherwise `policy.strikes` must fall
in the window. `PenaltyPolicy` defaults to two strikes within three epochs;
zero in either field disables cooldown. The orchestrator checks it after
exact-duplicate detection and stops a cooled proposal as `COOLDOWN` before
declaration, pinning or any evaluator call. `COOLDOWN` is not terminal: the
shape is eligible again once its strikes leave the window.
