import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { parseEnvelope, type Envelope, type LoopEvent } from "./events";
import { applyEnvelope, initialRunState, reduceEnvelopes } from "./reducer";

function fixture(): Envelope[] {
  return readFileSync(path.join(__dirname, "../fixtures/curve-fit/events.jsonl"), "utf8")
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const r = parseEnvelope(line);
      if (!r.ok) throw new Error(r.error);
      return r.envelope;
    });
}

const env = (seq: number, event: LoopEvent): Envelope => ({ seq, elapsed_ms: seq, event });

describe("reducer", () => {
  it("folds the fixture run into goal, lineage, series and decisions", () => {
    const s = reduceEnvelopes(fixture());
    expect(s.goal).toEqual({ metric: "rmse", direction: "minimize", target: 0.05 });
    expect(s.runId).toBe("curve-fit-1");
    expect(s.finished).toEqual({ reason: "goal_reached" });
    expect(s.incumbent).toBe("3c2b1a0f9e8d");
    expect(s.incumbentMetric).toBe(0.04);
    expect(s.lineage.map((l) => [l.id, l.epoch, l.metric])).toEqual([
      ["a1b2c3d4e5f6", 0, 0.42],
      ["0f9e8d7c6b5a", 1, 0.11],
      ["3c2b1a0f9e8d", 3, 0.04],
    ]);
    expect(s.series).toEqual([
      { epoch: 0, metric: 0.42 },
      { epoch: 1, metric: 0.11 },
      { epoch: 2, metric: 0.11 },
      { epoch: 3, metric: 0.04 },
    ]);
    const decisions = s.candidateOrder.map((id) => s.candidates[id]!.decision);
    expect(decisions).toEqual([null, "accept", "reject", "inconclusive", "accept"]);
    expect(s.gaps).toBe(0);
  });

  it("pairs proposals with candidates by the id in ideated", () => {
    const s = reduceEnvelopes(fixture());
    expect(s.proposals.map((p) => [p.proposalId, p.candidate])).toEqual([
      ["p1-0", "0f9e8d7c6b5a"],
      ["p2-0", "5e4d3c2b1a09"],
      ["p2-1", "9a8b7c6d5e4f"],
      ["p3-0", "3c2b1a0f9e8d"],
      ["p3-1", null],
    ]);
    expect(s.candidates["5e4d3c2b1a09"]).toMatchObject({ proposalId: "p2-0", lower: -0.11, upper: -0.05 });
    expect(s.candidates["a1b2c3d4e5f6"]!.proposalId).toBeNull();
  });

  it("pairs by id even when candidates run out of proposal order", () => {
    let s = initialRunState();
    s = applyEnvelope(s, env(0, { kind: "epoch_started", epoch: 1, incumbent: "inc" }));
    s = applyEnvelope(s, env(1, { kind: "ideated", epoch: 1, proposal_id: "p0", parent: "inc", candidate: "c0", rationale: "r", hypothesis: "h" }));
    s = applyEnvelope(s, env(2, { kind: "ideated", epoch: 1, proposal_id: "p1", parent: "inc", candidate: "c1", rationale: "r", hypothesis: "h" }));
    s = applyEnvelope(s, env(3, { kind: "ideated", epoch: 1, proposal_id: "p2", parent: "inc", candidate: null, rationale: "r", hypothesis: "h" }));
    s = applyEnvelope(s, env(4, { kind: "executed", epoch: 1, candidate: "inc", seed: 1, metric: 1, ok: true, duration_ms: 1 }));
    s = applyEnvelope(s, env(5, { kind: "executed", epoch: 1, candidate: "c1", seed: 1, metric: 1, ok: false, duration_ms: 1 }));
    s = applyEnvelope(s, env(6, { kind: "executed", epoch: 1, candidate: "c0", seed: 1, metric: 1, ok: true, duration_ms: 1 }));
    expect(s.candidates.c1).toMatchObject({ proposalId: "p1", runs: 1, okRuns: 0 });
    expect(s.candidates.c0!.proposalId).toBe("p0");
    expect(s.candidates.inc!.proposalId).toBeNull();
    expect(s.proposals.find((p) => p.proposalId === "p2")!.candidate).toBeNull();
  });

  it("links a candidate executed before its proposal is ideated", () => {
    let s = initialRunState();
    s = applyEnvelope(s, env(0, { kind: "executed", epoch: 1, candidate: "c", seed: 1, metric: 1, ok: true, duration_ms: 1 }));
    expect(s.candidates.c!.proposalId).toBeNull();
    s = applyEnvelope(s, env(1, { kind: "ideated", epoch: 1, proposal_id: "p", parent: "i", candidate: "c", rationale: "r", hypothesis: "h" }));
    expect(s.candidates.c!.proposalId).toBe("p");
  });

  it("tracks the stage of the latest stage-bearing event", () => {
    const events = fixture();
    const s = reduceEnvelopes(events.slice(0, 8));
    expect(s.stage).toBe("measure");
    const after = reduceEnvelopes(events.slice(0, 11));
    expect(after.stage).toBe("improve");
    expect(after.finished).toBeNull();
  });

  it("ignores replayed seqs and counts gaps", () => {
    const events = fixture();
    let s = reduceEnvelopes(events.slice(0, 4));
    const same = applyEnvelope(s, events[2]!);
    expect(same).toBe(s);
    s = applyEnvelope(s, events[6]!);
    expect(s.gaps).toBe(2);
  });

  it("keeps null metrics as null", () => {
    let s = initialRunState();
    s = applyEnvelope(s, env(0, { kind: "measured", epoch: 0, candidate: "b", mean: null, failures: 1, seeds: 1 }));
    expect(s.incumbent).toBe("b");
    expect(s.incumbentMetric).toBeNull();
    expect(s.series).toEqual([{ epoch: 0, metric: null }]);
    s = applyEnvelope(s, env(1, { kind: "run_finished", reason: "error", incumbent: "b", incumbent_metric: null }));
    expect(s.finished).toEqual({ reason: "error" });
  });

  it("does not mutate its input", () => {
    const s0 = reduceEnvelopes(fixture().slice(0, 5));
    const snapshot = JSON.stringify(s0);
    applyEnvelope(s0, fixture()[5]!);
    expect(JSON.stringify(s0)).toBe(snapshot);
  });
  it("keeps a candidate's interval units, metric unless the event says otherwise", () => {
    const scored = (candidate: string, units?: "metric" | "quality"): LoopEvent => ({
      kind: "scored",
      epoch: 1,
      candidate,
      decision: "accept",
      estimate: 0.5,
      lower: 0.25,
      upper: 0.75,
      ...(units ? { units } : {}),
    });
    const s = reduceEnvelopes([env(0, scored("q", "quality")), env(1, scored("m"))]);
    expect(s.candidates.q!.units).toBe("quality");
    expect(s.candidates.m!.units).toBe("metric");
  });
});
