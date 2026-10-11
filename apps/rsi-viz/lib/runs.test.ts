import { mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { discoverRuns } from "./runs";

let root: string;

function touch(rel: string, mtime: number) {
  const file = path.join(root, rel);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, "{}");
  utimesSync(file, mtime, mtime);
}

beforeEach(() => {
  root = mkdtempSync(path.join(tmpdir(), "rsi-viz-runs-"));
});

afterEach(() => rmSync(root, { recursive: true, force: true }));

describe("discoverRuns", () => {
  it("finds trace and snapshot runs, newest first", async () => {
    touch("rsi-script-loop/run-1/events.jsonl", 2000);
    touch("rsi-script-loop/run-1/ledger.json", 1000);
    touch("hive/experiment-report.json", 3000);
    touch("hive/notes.txt", 9000);
    touch("hive/blueprint.generation-2.json", 1000);
    touch("unrelated/ledger.json", 5000);
    const runs = await discoverRuns(root);
    expect(runs).toEqual([
      {
        id: "hive",
        kind: "snapshot",
        updatedAt: new Date(3000 * 1000).toISOString(),
        files: ["blueprint.generation-2.json", "experiment-report.json"],
      },
      {
        id: "rsi-script-loop/run-1",
        kind: "trace",
        updatedAt: new Date(2000 * 1000).toISOString(),
        files: ["events.jsonl", "ledger.json"],
      },
    ]);
  });

  it("treats the root itself as a run with id '.'", async () => {
    touch("events.jsonl", 1000);
    expect((await discoverRuns(root)).map((r) => r.id)).toEqual(["."]);
  });

  it("stops at the depth bound and skips build and hidden directories", async () => {
    touch("a/b/c/events.jsonl", 1000);
    touch("a/b/c/d/events.jsonl", 1000);
    touch("debug/x/events.jsonl", 1000);
    touch(".hidden/events.jsonl", 1000);
    expect((await discoverRuns(root, 3)).map((r) => r.id)).toEqual(["a/b/c"]);
  });

  it("returns nothing for a missing root", async () => {
    expect(await discoverRuns(path.join(root, "missing"))).toEqual([]);
  });
});
