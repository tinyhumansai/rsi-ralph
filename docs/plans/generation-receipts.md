# Generation occurrence implementation

Spec: [generation receipts](../specs/generation-receipts.md).

1. Add a regression asserting malformed and duplicate formatter occurrences
   survive journal reopen, with distinct IDs and no raw payloads; demonstrate red.
2. Add typed safe receipts and atomic sidecar storage, with validation of identity,
   call order, item offsets, pending calls and terminal dispositions.
3. Journal call intent and callback dispositions around actual formatter calls.
4. Test failures, interruption, corruption, privacy and Engine integration; run
   formatting, clippy, build, tests, rustdoc and the per-file coverage gate.
