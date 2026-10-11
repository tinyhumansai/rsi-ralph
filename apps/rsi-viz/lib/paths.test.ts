import { mkdirSync, mkdtempSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { isSnapshotFile, PathRejected, resolveInRoot, runRoot, validateRunId } from "./paths";

let base: string;
let root: string;

beforeEach(() => {
  base = realpathSync(mkdtempSync(path.join(tmpdir(), "rsi-viz-paths-")));
  root = path.join(base, "root");
  mkdirSync(path.join(root, "runs", "a"), { recursive: true });
  writeFileSync(path.join(root, "runs", "a", "events.jsonl"), "");
  writeFileSync(path.join(base, "secret.json"), "{}");
});

afterEach(() => rmSync(base, { recursive: true, force: true }));

describe("validateRunId", () => {
  it("splits a nested relative id", () => {
    expect(validateRunId("runs/a")).toEqual(["runs", "a"]);
    expect(validateRunId(".")).toEqual([]);
  });

  it.each([
    ["..", "segment"],
    ["runs/../..", "segment"],
    ["runs//a", "segment"],
    ["./runs", "segment"],
    ["/etc", "absolute"],
    ["C:/x", "absolute"],
    ["runs\\a", "backslash"],
    ["runs\0a", "NUL"],
  ])("rejects %j", (id, reason) => {
    expect(() => validateRunId(id)).toThrow(PathRejected);
    expect(() => validateRunId(id)).toThrow(reason);
  });
});

describe("resolveInRoot", () => {
  it("resolves a run directory and a file inside it", async () => {
    expect(await resolveInRoot(root, "runs/a")).toBe(path.join(root, "runs", "a"));
    expect(await resolveInRoot(root, "runs/a", "events.jsonl")).toBe(path.join(root, "runs", "a", "events.jsonl"));
  });

  it("rejects a file name that is not a bare name", async () => {
    await expect(resolveInRoot(root, "runs/a", "../a/events.jsonl")).rejects.toThrow(PathRejected);
    await expect(resolveInRoot(root, "runs", "..")).rejects.toThrow(PathRejected);
  });

  it("rejects a symlink that escapes the root", async () => {
    symlinkSync(path.join(base, "secret.json"), path.join(root, "runs", "a", "ledger.json"));
    symlinkSync(base, path.join(root, "escape"));
    await expect(resolveInRoot(root, "runs/a", "ledger.json")).rejects.toThrow("escapes");
    await expect(resolveInRoot(root, "escape")).rejects.toThrow("escapes");
  });

  it("allows a symlink that stays inside the root", async () => {
    symlinkSync(path.join(root, "runs", "a"), path.join(root, "latest"));
    expect(await resolveInRoot(root, "latest", "events.jsonl")).toBe(path.join(root, "runs", "a", "events.jsonl"));
  });

  it("surfaces a missing path as ENOENT", async () => {
    await expect(resolveInRoot(root, "runs/missing")).rejects.toMatchObject({ code: "ENOENT" });
  });
});

describe("isSnapshotFile", () => {
  it("allows the listed snapshots and generation files only", () => {
    for (const name of ["archive.json", "epoch.json", "experiment-report.json", "ledger.json", "deployment.json"]) {
      expect(isSnapshotFile(name)).toBe(true);
    }
    expect(isSnapshotFile("blueprint.generation-3.json")).toBe(true);
    expect(isSnapshotFile("events.jsonl")).toBe(false);
    expect(isSnapshotFile("secrets.json")).toBe(false);
    expect(isSnapshotFile("../archive.json")).toBe(false);
    expect(isSnapshotFile("x.generation-.json")).toBe(false);
  });
});

describe("runRoot", () => {
  it("defaults to the workspace target and honours RSI_RUN_ROOT", () => {
    const previous = process.env.RSI_RUN_ROOT;
    try {
      delete process.env.RSI_RUN_ROOT;
      expect(runRoot()).toBe(path.resolve(process.cwd(), "../../target"));
      process.env.RSI_RUN_ROOT = "./fixtures";
      expect(runRoot()).toBe(path.resolve(process.cwd(), "fixtures"));
    } finally {
      if (previous === undefined) delete process.env.RSI_RUN_ROOT;
      else process.env.RSI_RUN_ROOT = previous;
    }
  });
});
