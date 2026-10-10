# Public failure evidence

`Observer::snapshot` reads the coordinator's transcript after its cursor and
all retained episode statuses. It advances the cursor only after both reads
succeed. These APIs are separate reads, so concurrent coordinator changes can
make the episode snapshot newer than the transcript snapshot.

Returned observations retain only sequence references, seat and episode
identities, opening sequences and redacted phases. Message bodies, private
readers, destinations and failure reasons are discarded.

`Dossier::build` combines an observation with explicit host token measurements,
node outcomes and memory health counters. It sorts and deduplicates evidence,
aggregates costs by seat and saturates counters on overflow. Conflicting node
outcomes retain both open and failed evidence rather than guessing chronology.
The caller determines the epoch window and supplies each turn once. Dossiers
contain no task text, oracle answers or implicit memory transactions.
