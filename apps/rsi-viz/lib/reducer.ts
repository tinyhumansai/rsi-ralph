/**
 * Folds loop trace envelopes into the state the run page renders.
 *
 * The reducer is pure: `applyEnvelope(state, envelope)` returns a new state
 * and never mutates its input, so it plugs straight into React's
 * `useReducer`. Envelopes with a `seq` at or below the last applied one are
 * ignored, which makes a replay after an SSE reconnect harmless.
 *
 * The trace does not carry an explicit proposal-to-candidate link. Within an
 * epoch, new candidates are matched to `ideated` proposals in order: the
 * first new candidate executed in epoch N belongs to the first proposal
 * ideated in epoch N, and so on. Re-runs of the incumbent are not candidates.
 */

import { stageOf, type Decision, type Envelope, type FinishReason, type Goal, type Stage } from "./events";

/** One proposal and, once known, the candidate built from it. */
export interface Proposal {
  epoch: number;
  proposalId: string;
  parent: string;
  rationale: string;
  hypothesis: string;
  candidate: string | null;
}

/** A candidate program the loop executed. */
export interface Candidate {
  id: string;
  epoch: number;
  proposalId: string | null;
  runs: number;
  okRuns: number;
  mean: number | null;
  failures: number;
  seeds: number;
  decision: Decision | null;
  estimate: number | null;
  lower: number | null;
  upper: number | null;
}

/** One incumbent in the promotion chain. */
export interface LineageEntry {
  id: string;
  epoch: number;
  metric: number | null;
}

export interface RunState {
  runId: string | null;
  subject: string | null;
  goal: Goal | null;
  /** Ring stage of the latest event that maps to one. */
  stage: Stage | null;
  epoch: number | null;
  incumbent: string | null;
  incumbentMetric: number | null;
  lineage: LineageEntry[];
  candidates: Record<string, Candidate>;
  candidateOrder: string[];
  proposals: Proposal[];
  /** Incumbent metric at the end of each epoch, in epoch order. */
  series: { epoch: number; metric: number | null }[];
  finished: { reason: FinishReason } | null;
  lastSeq: number;
  elapsedMs: number;
  /** Count of seq numbers skipped between applied envelopes. */
  gaps: number;
}

export function initialRunState(): RunState {
  return {
    runId: null,
    subject: null,
    goal: null,
    stage: null,
    epoch: null,
    incumbent: null,
    incumbentMetric: null,
    lineage: [],
    candidates: {},
    candidateOrder: [],
    proposals: [],
    series: [],
    finished: null,
    lastSeq: -1,
    elapsedMs: 0,
    gaps: 0,
  };
}

function upsertSeries(series: RunState["series"], epoch: number, metric: number | null): RunState["series"] {
  const next = series.filter((p) => p.epoch !== epoch);
  next.push({ epoch, metric });
  next.sort((a, b) => a.epoch - b.epoch);
  return next;
}

function setIncumbent(state: RunState, id: string, epoch: number, metric: number | null): RunState {
  const last = state.lineage[state.lineage.length - 1];
  const lineage =
    last && last.id === id
      ? metric !== null && last.metric === null
        ? [...state.lineage.slice(0, -1), { ...last, metric }]
        : state.lineage
      : [...state.lineage, { id, epoch, metric }];
  const incumbentMetric = id === state.incumbent ? (metric ?? state.incumbentMetric) : metric;
  return { ...state, incumbent: id, incumbentMetric, lineage };
}

/** Ensures a candidate entry exists, linking it to the next unmatched proposal. */
function touchCandidate(state: RunState, id: string, epoch: number): RunState {
  if (state.candidates[id]) return state;
  let proposalId: string | null = null;
  let proposals = state.proposals;
  if (epoch > 0 && id !== state.incumbent) {
    const index = proposals.findIndex((p) => p.epoch === epoch && p.candidate === null);
    if (index >= 0) {
      const proposal = proposals[index]!;
      proposalId = proposal.proposalId;
      proposals = proposals.map((p, i) => (i === index ? { ...p, candidate: id } : p));
    }
  }
  const candidate: Candidate = {
    id,
    epoch,
    proposalId,
    runs: 0,
    okRuns: 0,
    mean: null,
    failures: 0,
    seeds: 0,
    decision: null,
    estimate: null,
    lower: null,
    upper: null,
  };
  return {
    ...state,
    proposals,
    candidates: { ...state.candidates, [id]: candidate },
    candidateOrder: [...state.candidateOrder, id],
  };
}

function patchCandidate(state: RunState, id: string, patch: Partial<Candidate>): RunState {
  const current = state.candidates[id];
  if (!current) return state;
  return { ...state, candidates: { ...state.candidates, [id]: { ...current, ...patch } } };
}

/** Applies one envelope. Returns `state` unchanged for a replayed `seq`. */
export function applyEnvelope(state: RunState, envelope: Envelope): RunState {
  if (envelope.seq <= state.lastSeq) return state;
  const gaps = state.gaps + Math.max(0, envelope.seq - state.lastSeq - 1);
  const event = envelope.event;
  let next: RunState = {
    ...state,
    lastSeq: envelope.seq,
    elapsedMs: envelope.elapsed_ms,
    gaps,
    stage: stageOf(event.kind) ?? state.stage,
  };

  switch (event.kind) {
    case "run_started":
      next = { ...next, runId: event.run_id, subject: event.subject, goal: event.goal };
      break;
    case "epoch_started":
      next = { ...next, epoch: event.epoch };
      if (next.incumbent === null) next = setIncumbent(next, event.incumbent, 0, null);
      break;
    case "observed":
      next = { ...next, epoch: event.epoch };
      next = setIncumbent(next, event.incumbent, event.epoch, event.incumbent_metric);
      break;
    case "ideated":
      next = {
        ...next,
        epoch: event.epoch,
        proposals: [
          ...next.proposals,
          {
            epoch: event.epoch,
            proposalId: event.proposal_id,
            parent: event.parent,
            rationale: event.rationale,
            hypothesis: event.hypothesis,
            candidate: null,
          },
        ],
      };
      break;
    case "executed": {
      next = touchCandidate({ ...next, epoch: event.epoch }, event.candidate, event.epoch);
      const c = next.candidates[event.candidate]!;
      next = patchCandidate(next, event.candidate, { runs: c.runs + 1, okRuns: c.okRuns + (event.ok ? 1 : 0) });
      break;
    }
    case "measured":
      next = touchCandidate({ ...next, epoch: event.epoch }, event.candidate, event.epoch);
      next = patchCandidate(next, event.candidate, {
        mean: event.mean,
        failures: event.failures,
        seeds: event.seeds,
      });
      if (event.epoch === 0) {
        // The baseline run: it is the first incumbent.
        if (next.incumbent === null || next.incumbent === event.candidate) {
          next = setIncumbent(next, event.candidate, 0, event.mean);
        }
        if (next.incumbent === event.candidate) next = { ...next, series: upsertSeries(next.series, 0, event.mean) };
      }
      break;
    case "scored":
      next = touchCandidate(next, event.candidate, event.epoch);
      next = patchCandidate(next, event.candidate, {
        decision: event.decision,
        estimate: event.estimate,
        lower: event.lower,
        upper: event.upper,
      });
      break;
    case "improved":
      next = setIncumbent(next, event.to, event.epoch, event.metric);
      next = { ...next, incumbentMetric: event.metric };
      break;
    case "epoch_finished":
      next = setIncumbent(next, event.incumbent, event.epoch, event.incumbent_metric);
      next = { ...next, series: upsertSeries(next.series, event.epoch, event.incumbent_metric) };
      break;
    case "run_finished":
      next = setIncumbent(next, event.incumbent, next.epoch ?? 0, event.incumbent_metric);
      next = { ...next, finished: { reason: event.reason } };
      break;
  }
  return next;
}

/** Folds a whole list of envelopes from the initial state. */
export function reduceEnvelopes(envelopes: Iterable<Envelope>): RunState {
  let state = initialRunState();
  for (const envelope of envelopes) state = applyEnvelope(state, envelope);
  return state;
}
