/**
 * Client-side state for one subscribed run: the folded run state plus the
 * stream bookkeeping the header needs (replay done, invalid lines, the last
 * time the trace changed). Pure, so it is tested without a browser.
 */

import { parseEnvelope, type Envelope } from "./events";
import { applyEnvelope, initialRunState, type RunState } from "./reducer";

export interface ViewState {
  run: RunState;
  /** The server finished replaying the existing file. */
  ready: boolean;
  /** Lines that did not parse as a known envelope. */
  invalid: number;
  /** Last error message from parsing, for display. */
  lastError: string | null;
  /** Epoch milliseconds when the trace last changed, if known. */
  lastActivityMs: number | null;
}

export type ViewAction =
  | { type: "batch"; data: string; now: number }
  | { type: "ready"; mtimeMs: number | null }
  | { type: "reset" };

export function initialViewState(): ViewState {
  return { run: initialRunState(), ready: false, invalid: 0, lastError: null, lastActivityMs: null };
}

/** Splits one SSE message (lines joined by `\n`) into envelopes. */
export function parseBatch(data: string): { envelopes: Envelope[]; invalid: number; lastError: string | null } {
  const envelopes: Envelope[] = [];
  let invalid = 0;
  let lastError: string | null = null;
  for (const line of data.split("\n")) {
    if (line.trim() === "") continue;
    const result = parseEnvelope(line);
    if (result.ok) envelopes.push(result.envelope);
    else {
      invalid += 1;
      lastError = result.error;
    }
  }
  return { envelopes, invalid, lastError };
}

export function viewReducer(state: ViewState, action: ViewAction): ViewState {
  switch (action.type) {
    case "batch": {
      const { envelopes, invalid, lastError } = parseBatch(action.data);
      let run = state.run;
      for (const envelope of envelopes) run = applyEnvelope(run, envelope);
      return {
        ...state,
        run,
        invalid: state.invalid + invalid,
        lastError: lastError ?? state.lastError,
        lastActivityMs: state.ready ? action.now : state.lastActivityMs,
      };
    }
    case "ready":
      return { ...state, ready: true, lastActivityMs: action.mtimeMs ?? state.lastActivityMs };
    case "reset":
      return initialViewState();
  }
}

/** How long without a change before an unfinished run is shown as idle. */
export const IDLE_AFTER_MS = 60_000;

export type RunStatus = "connecting" | "live" | "idle" | "finished";

export function runStatus(state: ViewState, now: number): RunStatus {
  if (state.run.finished) return "finished";
  if (!state.ready) return "connecting";
  if (state.lastActivityMs === null || now - state.lastActivityMs > IDLE_AFTER_MS) return "idle";
  return "live";
}
