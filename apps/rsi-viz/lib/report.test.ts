import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import { asExperimentReport, cellKey, groupTrials } from "./report";

const raw: unknown = JSON.parse(
  readFileSync(path.join(__dirname, "../fixtures/hive-2026-10-11/experiment-report.json"), "utf8"),
);

describe("experiment report", () => {
  it("recognises the real report and rejects other JSON", () => {
    expect(asExperimentReport(raw)).not.toBeNull();
    expect(asExperimentReport({ trials: [{ arm: 1 }] })).toBeNull();
    expect(asExperimentReport({})).toBeNull();
    expect(asExperimentReport(null)).toBeNull();
  });

  it("groups 24 trials into a 2 × 3 grid with summed counts", () => {
    const report = asExperimentReport(raw)!;
    const { arms, ablations, cells } = groupTrials(report.trials);
    expect(arms).toEqual(["Original", "Evolved"]);
    expect(ablations).toEqual(["Full", "MemoryOff", "RetreatOff"]);
    let trials = 0;
    for (const cell of cells.values()) trials += cell.trials;
    expect(trials).toBe(24);
    const evolvedRetreatOff = cells.get(cellKey("Evolved", "RetreatOff"))!;
    expect(evolvedRetreatOff.failed).toBeGreaterThan(0);
    expect(evolvedRetreatOff.anchored.every((t) => t.anchor_gain !== null)).toBe(true);
  });
});
