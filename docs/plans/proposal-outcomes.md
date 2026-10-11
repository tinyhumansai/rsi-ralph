# Proposal outcome implementation

Spec: [selection-only outcomes](../specs/proposal-outcomes.md).

1. Extend the actual epoch evaluation regression to freeze priors and project
   selection evidence across reopen; confirm the unimplemented API fails.
2. Retain formatter occurrence links, optional provider identity and generation
   seed in backward-compatible private journal fields. Validate bindings on replay.
3. Add safe dataset projection and prior registration, matching exact archive
   evidence and excluding confirmation/anchor values and duplicate inheritance.
4. Implement Brier, tied-rank Spearman and censoring-aware precision at k.
5. Exercise numeric cases, malformed input, legacy journals, model changes,
   interrupted evaluation and private-data exclusion. Run the four contract
   checks, strict rustdoc, per-file coverage and an offline example.
