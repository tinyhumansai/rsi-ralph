# Creative retreat transport

`build_retreat_request` emits the actual tinybackrooms `POST /v1/generate` JSON
contract: `input`, `output` and `config`. The request selects `critic_pair`,
comparative/judge/originality scorers, three ranked items, hard turn/time limits
and `project:rsi-<hive-id>/agent:retreat` memory routing. The epoch seeds the draw.
The builder deserializes and validates against the actual vendored tinybackrooms
`GenerateRequest`, including compiling the output schema. `typed_retreat_request`
returns that validated service type.

The prompt contains a public package projection and redacted dossier evidence.
Constitution, factory configuration, goal acceptance criteria, unreferenced or
protected documents and protected memory namespaces are omitted. Known private
canary/configuration strings, goal acceptance criteria, hard constraints and
read-only memory identities are scrubbed from remaining public text. Canonical
JSON frontmatter is stripped; protected frontmatter fields are also added to
the withheld strings so copied values cannot escape through another document.
Malformed frontmatter rejects the request. Rejected
seeds (`PriorIdea`) contain only epoch, class, operation count, a whitelisted
reason and a coarse `outcome`: `crashed`, `regressed`, `flat`, `cooled`,
`denied` or `null`. Rejected patch bodies, untrusted rationale and measured
deltas or intervals never reach the model. `build_retreat_request` derives the
outcome from the reason; a host that knows a stop was a confident regression
passes its own seeds to `build_retreat_request_with_priors`.

`RetreatTransport` is implemented by the host's HTTP client. It sends the JSON
unchanged and handles authentication, service errors and timeout. This module
does not install or claim a persistent tinybackrooms memory adapter.

`generate_retreat_with_engine` runs the real `tinybackrooms::Engine` against
caller-supplied model and memory ports. It executes proposal/critic conversations,
formatter extraction, all configured scorers and ranked selection, then strictly
decodes the resulting items. Deterministic tests exercise this entire pipeline
with `ScriptedConversant`; real model credentials are unnecessary for tests.

The output schema includes all actual language operation tags (`json`,
`document`, `prompt`, `context`, `learning`) and denies unknown proposal fields.
`decode_retreat_response` reads `items[].value`, preserves ranking and counts
invalid items without publishing their raw text. `generate_retreat` returns
only the valid proposals; use the decoder directly for invalid-item auditing.
Retreat rankings never replace host validation, paired evaluation or approval.
