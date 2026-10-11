# P1 evidence journal implementation plan

Contract: [durable epoch evidence](../specs/epoch-evidence.md).

1. Add behavior-first journal tests for reopened usage/check evidence, idempotent
   callbacks, conflicting bindings, open/interrupted turns, repeated epochs and
   failed persistence. Confirm failures before implementing the API.
2. Implement typed public turn/epoch receipts and an exclusive atomic JSON
   journal. Keep persistence and validation in focused files. Expose only
   identities, counters and existing redacted verification evidence.
3. Add a host verification observer invoked before completion. Test persistence
   failure parks the real episode, preventing success without durable evidence.
4. Opt the controlled factory into the journal: persist invocation intent before
   dispatch, usage immediately after provider return, checker evidence before
   completion, and final turn status before returning the host outcome.
5. Add an offline two-epoch host example consuming Coordinator notifications;
   reopen coordinator and journal, repeat callbacks/checkpoints, and assert
   measured usage and observed checker results are neither lost nor recounted.
6. Update the P1 status/audit and example instructions. Run the four cargo checks,
   strict rustdoc, offline example and per-file coverage before opening the PR.
