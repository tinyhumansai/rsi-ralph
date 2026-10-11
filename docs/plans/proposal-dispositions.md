# Proposal disposition implementation

Contract: [safe duplicate history](../specs/proposal-dispositions.md).

1. Add accepted/rejected-history regression cases to the actual normalized
   proposal flow. Confirm the missing projection fails before implementation.
2. Add versioned safe report types and a chronological archive projection.
   Retain coarse reason compatibility and explicit unresolved legacy cases.
3. Test restart, rollback, prepared-only and duplicate chains, unknown reasons,
   and later acceptance without retrospective changes. Pin wire serialization.
4. Document the narrow implemented boundary and run the four Rust checks,
   strict documentation and per-file coverage.
