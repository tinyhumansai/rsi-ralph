import { describe, expect, it } from "vitest";
import { initialViewState, IDLE_AFTER_MS, parseBatch, runStatus, viewReducer } from "./view";

const started =
  '{"seq":0,"elapsed_ms":0,"event":{"kind":"run_started","run_id":"r","subject":"s","goal":{"metric":"m","direction":"maximize","target":null}}}';
const observed =
  '{"seq":1,"elapsed_ms":5,"event":{"kind":"observed","epoch":1,"incumbent":"a","incumbent_metric":1,"history":0,"rejected":0}}';
const finished =
  '{"seq":2,"elapsed_ms":9,"event":{"kind":"run_finished","reason":"stalled","incumbent":"a","incumbent_metric":1}}';

describe("parseBatch", () => {
  it("splits a joined batch and counts bad lines", () => {
    const r = parseBatch(`${started}\n\nnot json\n${observed}`);
    expect(r.envelopes.map((e) => e.seq)).toEqual([0, 1]);
    expect(r.invalid).toBe(1);
    expect(r.lastError).toBe("invalid json");
  });
});

describe("viewReducer", () => {
  it("folds batches, marks ready, and resets", () => {
    let s = viewReducer(initialViewState(), { type: "batch", data: started, now: 10 });
    expect(s.run.goal?.direction).toBe("maximize");
    expect(s.lastActivityMs).toBeNull();
    s = viewReducer(s, { type: "ready", mtimeMs: 1000 });
    expect(s.ready).toBe(true);
    expect(s.lastActivityMs).toBe(1000);
    s = viewReducer(s, { type: "batch", data: `${observed}\n{}`, now: 2000 });
    expect(s.run.stage).toBe("observe");
    expect(s.lastActivityMs).toBe(2000);
    expect(s.invalid).toBe(1);
    expect(viewReducer(s, { type: "reset" })).toEqual(initialViewState());
  });

  it("keeps the previous activity time when ready has no mtime", () => {
    const s = viewReducer(initialViewState(), { type: "ready", mtimeMs: null });
    expect(s.lastActivityMs).toBeNull();
  });
});

describe("runStatus", () => {
  it("reports connecting, live, idle, and finished", () => {
    let s = initialViewState();
    expect(runStatus(s, 0)).toBe("connecting");
    s = viewReducer(s, { type: "ready", mtimeMs: 1000 });
    expect(runStatus(s, 1000 + IDLE_AFTER_MS)).toBe("live");
    expect(runStatus(s, 1001 + IDLE_AFTER_MS)).toBe("idle");
    expect(runStatus(viewReducer(s, { type: "ready", mtimeMs: null }), 0)).toBe("live");
    expect(runStatus(viewReducer(initialViewState(), { type: "ready", mtimeMs: null }), 0)).toBe("idle");
    s = viewReducer(s, { type: "batch", data: finished, now: 5 });
    expect(runStatus(s, 0)).toBe("finished");
  });
});
