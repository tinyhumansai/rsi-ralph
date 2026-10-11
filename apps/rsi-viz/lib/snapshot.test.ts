import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { PathRejected } from "./paths";
import { hasTrace, readSnapshot } from "./snapshot";

let root: string;

beforeEach(() => {
  root = mkdtempSync(path.join(tmpdir(), "rsi-viz-snap-"));
  mkdirSync(path.join(root, "run", "epoch.json", "x"), { recursive: true });
  writeFileSync(path.join(root, "run", "archive.json"), '{"a":1}');
  writeFileSync(path.join(root, "run", "notes.json"), "{}");
  writeFileSync(path.join(root, "run", "events.jsonl"), "");
});

afterEach(() => rmSync(root, { recursive: true, force: true }));

describe("readSnapshot", () => {
  it("returns an allowlisted file", async () => {
    expect(await readSnapshot(root, "run", "archive.json")).toBe('{"a":1}');
  });

  it("refuses files outside the allowlist, directories, and traversal", async () => {
    await expect(readSnapshot(root, "run", "notes.json")).rejects.toThrow(PathRejected);
    await expect(readSnapshot(root, "run", "events.jsonl")).rejects.toThrow(PathRejected);
    await expect(readSnapshot(root, "run", "epoch.json")).rejects.toThrow("not a regular file");
    await expect(readSnapshot(root, "../run", "archive.json")).rejects.toThrow(PathRejected);
  });

  it("reports a missing allowlisted file as ENOENT", async () => {
    await expect(readSnapshot(root, "run", "ledger.json")).rejects.toMatchObject({ code: "ENOENT" });
  });
});

describe("hasTrace", () => {
  it("detects events.jsonl and rethrows rejected ids", async () => {
    expect(await hasTrace(root, "run")).toBe(true);
    expect(await hasTrace(root, "missing")).toBe(false);
    await expect(hasTrace(root, "..")).rejects.toThrow(PathRejected);
  });
});
