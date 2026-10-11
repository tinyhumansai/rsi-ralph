# Safe versioned duplicate dispositions

Related: issues #1 and #6, scoring reject taxonomy.

Expose a version-one, read-only projection of replay-validated archive proposal
stops. Keep the existing twelve `ReasonCode` wire values and archive format.
Rows retain archive event index, epoch, recognized coarse reason and an optional
exact-duplicate detail: `DUP_ACCEPTED`, `DUP_REJECTED` or `DUP_UNRESOLVED`.

Classify each duplicate from earlier events only. A matching accepted language
record supplies accepted history; rejected records or recognized terminal host
rejections supply rejected history. Prepared-only candidates, infrastructure-only stops without a rejected
language record, unknown legacy reasons and content aliases without the exact
patch stay unresolved. A formal rejected record describes an archive decision,
including interrupted/denied attempts; it is not a selection eligibility label. Duplicate rows do not manufacture rejected history for later rows.
Accepted historical records continue to count after rollback. Never relabel an
earlier occurrence using a later acceptance.

The output excludes patch contents, arbitrary reason text, rationale and
transcript payloads. Every proposal stop receives a row, including unknown
legacy reasons, which retain an absent coarse reason. This metadata supplies no
selection eligibility or measurements and must not change promotion decisions.
The remaining formalization, reach, contamination, constraint and semantic
subtypes remain separate work; this slice does not claim all seventeen codes.
