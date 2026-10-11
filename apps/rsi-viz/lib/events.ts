/**
 * Wire types for the loop trace (`events.jsonl`).
 *
 * These mirror the schema owned by the Rust `trace` module exactly: one
 * envelope per line, `event.kind` in snake_case, and optional numbers encoded
 * as `null` rather than omitted. Do not add fields here that the writer does
 * not emit.
 */

/** Whether a smaller or larger metric value is better. */
export type Direction = "minimize" | "maximize";

/** The optimisation goal a run was started with. */
export interface Goal {
  metric: string;
  direction: Direction;
  target: number | null;
}

/** Outcome of the paired comparison between a candidate and the incumbent. */
export type Decision = "accept" | "reject" | "inconclusive";

/** Units of a scored interval: the goal metric, or scorer qualities in [0, 1]. */
export type IntervalUnits = "metric" | "quality";

/** Why the loop stopped. */
export type FinishReason =
  | "goal_reached"
  | "max_epochs"
  | "stalled"
  | "budget"
  | "error"
  | "provider_refused";

export interface RunStarted {
  kind: "run_started";
  run_id: string;
  subject: string;
  goal: Goal;
}

export interface EpochStarted {
  kind: "epoch_started";
  epoch: number;
  incumbent: string;
}

export interface Observed {
  kind: "observed";
  epoch: number;
  incumbent: string;
  incumbent_metric: number | null;
  history: number;
  rejected: number;
}

export interface Ideated {
  kind: "ideated";
  epoch: number;
  proposal_id: string;
  parent: string;
  /** Content id the proposal materializes into; null if it was rejected first. */
  candidate: string | null;
  rationale: string;
  hypothesis: string;
  /** Short label of the proposal's technique; absent or null when unlabelled. */
  approach?: string | null;
}

export interface Executed {
  kind: "executed";
  epoch: number;
  candidate: string;
  seed: number;
  metric: number | null;
  ok: boolean;
  duration_ms: number;
}

export interface Measured {
  kind: "measured";
  epoch: number;
  candidate: string;
  mean: number | null;
  failures: number;
  seeds: number;
}

export interface Scored {
  kind: "scored";
  epoch: number;
  candidate: string;
  decision: Decision;
  estimate: number;
  lower: number;
  upper: number;
  /** Absent for metric units, which older traces never wrote. */
  units?: IntervalUnits;
}

export interface Improved {
  kind: "improved";
  epoch: number;
  from: string;
  to: string;
  metric: number;
}

export interface EpochFinished {
  kind: "epoch_finished";
  epoch: number;
  incumbent: string;
  incumbent_metric: number | null;
}

/** The model provider refused calls in an epoch whose hive changed nothing. */
export interface ProviderRefused {
  kind: "provider_refused";
  epoch: number;
  /** Model calls the hive sent this epoch. */
  calls: number;
  /** Of those, calls refused or never answered. */
  refused: number;
  /** HTTP status of the latest refusal; 502 when the provider was unreachable. */
  status: number;
  /** The provider's message for the latest refusal. */
  message: string;
}

export interface RunFinished {
  kind: "run_finished";
  reason: FinishReason;
  incumbent: string;
  incumbent_metric: number | null;
}

/** Every event the loop can emit. */
export type LoopEvent =
  | RunStarted
  | EpochStarted
  | Observed
  | Ideated
  | Executed
  | Measured
  | Scored
  | Improved
  | EpochFinished
  | ProviderRefused
  | RunFinished;

/** One line of `events.jsonl`. */
export interface Envelope {
  seq: number;
  elapsed_ms: number;
  event: LoopEvent;
}

/** The stages drawn on the loop ring, in loop order. */
export const STAGES = ["observe", "ideate", "execute", "measure", "score", "improve"] as const;
export type Stage = (typeof STAGES)[number];

/** Maps an event kind to the ring stage it lights up, or `null` for none. */
export function stageOf(kind: LoopEvent["kind"]): Stage | null {
  switch (kind) {
    case "observed":
      return "observe";
    case "ideated":
      return "ideate";
    case "executed":
      return "execute";
    case "measured":
      return "measure";
    case "scored":
      return "score";
    case "improved":
      return "improve";
    default:
      return null;
  }
}

type FieldType = "string" | "string|null" | "number" | "number|null" | "boolean" | "goal";

const FIELDS: Record<LoopEvent["kind"], Record<string, FieldType>> = {
  run_started: { run_id: "string", subject: "string", goal: "goal" },
  epoch_started: { epoch: "number", incumbent: "string" },
  observed: {
    epoch: "number",
    incumbent: "string",
    incumbent_metric: "number|null",
    history: "number",
    rejected: "number",
  },
  ideated: {
    epoch: "number",
    proposal_id: "string",
    parent: "string",
    candidate: "string|null",
    rationale: "string",
    hypothesis: "string",
  },
  executed: {
    epoch: "number",
    candidate: "string",
    seed: "number",
    metric: "number|null",
    ok: "boolean",
    duration_ms: "number",
  },
  measured: {
    epoch: "number",
    candidate: "string",
    mean: "number|null",
    failures: "number",
    seeds: "number",
  },
  scored: {
    epoch: "number",
    candidate: "string",
    decision: "string",
    estimate: "number",
    lower: "number",
    upper: "number",
  },
  improved: { epoch: "number", from: "string", to: "string", metric: "number" },
  epoch_finished: { epoch: "number", incumbent: "string", incumbent_metric: "number|null" },
  provider_refused: {
    epoch: "number",
    calls: "number",
    refused: "number",
    status: "number",
    message: "string",
  },
  run_finished: { reason: "string", incumbent: "string", incumbent_metric: "number|null" },
};

const DECISIONS: readonly string[] = ["accept", "reject", "inconclusive"];
const UNITS: readonly string[] = ["metric", "quality"];
const REASONS: readonly string[] = ["goal_reached", "max_epochs", "stalled", "budget", "error", "provider_refused"];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function matches(value: unknown, type: FieldType): boolean {
  switch (type) {
    case "string":
      return typeof value === "string";
    case "string|null":
      return value === null || typeof value === "string";
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    case "number|null":
      return value === null || (typeof value === "number" && Number.isFinite(value));
    case "boolean":
      return typeof value === "boolean";
    case "goal":
      return (
        isRecord(value) &&
        typeof value.metric === "string" &&
        (value.direction === "minimize" || value.direction === "maximize") &&
        matches(value.target, "number|null")
      );
  }
}

/** Result of parsing one line: the envelope, or a reason it was rejected. */
export type ParseResult = { ok: true; envelope: Envelope } | { ok: false; error: string };

/**
 * Parses and validates one `events.jsonl` line.
 *
 * Unknown kinds, missing fields, and wrongly typed fields are rejected so the
 * reducer only ever sees well-formed events.
 */
export function parseEnvelope(line: string): ParseResult {
  let raw: unknown;
  try {
    raw = JSON.parse(line);
  } catch {
    return { ok: false, error: "invalid json" };
  }
  if (!isRecord(raw)) return { ok: false, error: "envelope is not an object" };
  if (!matches(raw.seq, "number") || !matches(raw.elapsed_ms, "number")) {
    return { ok: false, error: "envelope missing seq or elapsed_ms" };
  }
  const event = raw.event;
  if (!isRecord(event) || typeof event.kind !== "string") {
    return { ok: false, error: "envelope missing event.kind" };
  }
  const fields = (FIELDS as Record<string, Record<string, FieldType> | undefined>)[event.kind];
  if (!fields) return { ok: false, error: `unknown event kind ${event.kind}` };
  for (const [name, type] of Object.entries(fields)) {
    if (!matches(event[name], type)) {
      return { ok: false, error: `${event.kind}.${name} is not ${type}` };
    }
  }
  if (event.kind === "scored" && !DECISIONS.includes(event.decision as string)) {
    return { ok: false, error: "scored.decision is not a known decision" };
  }
  if (event.kind === "scored" && event.units !== undefined && !UNITS.includes(event.units as string)) {
    return { ok: false, error: "scored.units is not known units" };
  }
  if (event.kind === "ideated" && event.approach !== undefined && !matches(event.approach, "string|null")) {
    return { ok: false, error: "ideated.approach is not string|null" };
  }
  if (event.kind === "run_finished" && !REASONS.includes(event.reason as string)) {
    return { ok: false, error: "run_finished.reason is not a known reason" };
  }
  return {
    ok: true,
    envelope: { seq: raw.seq as number, elapsed_ms: raw.elapsed_ms as number, event: event as unknown as LoopEvent },
  };
}
