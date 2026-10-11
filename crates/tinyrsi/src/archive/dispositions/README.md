# Chronological proposal dispositions

`Archive::proposal_dispositions` projects replay-validated events into a safe
version-one report. It retains event index, epoch, recognized `ReasonCode` and
an exact-duplicate subtype. Both historical Rust reason spelling and public
serde reason spelling are recognized; arbitrary legacy strings are omitted.

`DUP_ACCEPTED` requires an earlier accepted language record with the exact
patch. `DUP_REJECTED` requires an earlier rejected language record or recognized
terminal proposal rejection. This includes approval denial, inconclusive
host stops and `HARMFUL` stops, where the candidate failed runs its incumbent
completed; it is an archive disposition, not a negative selection label.
Infrastructure failure alone, a prepared-only design, unknown legacy reasons
and content aliases without matching operations produce `DUP_UNRESOLVED`.
Duplicate chains never create new rejected-history evidence. Accepted history
survives rollback. Classification uses the prefix before each stop and does not
change when a later proposal succeeds.

No patches, free-form reasons, rationales or transcript payloads are exposed.
Existing archive formats, coarse outcomes, promotion decisions and proposal
selection labels remain unchanged. Semantic grounding, formalization subtypes,
reach/contamination codes and constraint/canary detail remain separate work.
