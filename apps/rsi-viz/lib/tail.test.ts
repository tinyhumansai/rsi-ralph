import { appendFileSync, mkdtempSync, renameSync, rmSync, truncateSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { FileTailer } from "./tail";

let dir: string;
let file: string;

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "rsi-viz-tail-"));
  file = path.join(dir, "events.jsonl");
});

afterEach(() => rmSync(dir, { recursive: true, force: true }));

function collect() {
  const lines: string[] = [];
  let resets = 0;
  const tailer = new FileTailer(file, {
    onLines: (batch) => lines.push(...batch),
    onReset: () => {
      resets += 1;
    },
    chunkBytes: 4,
  });
  return { tailer, lines, resets: () => resets };
}

describe("FileTailer", () => {
  it("waits quietly for a file that does not exist yet", async () => {
    const { tailer, lines } = collect();
    await tailer.poll();
    expect(lines).toEqual([]);
    writeFileSync(file, "a\n");
    await tailer.poll();
    expect(lines).toEqual(["a"]);
  });

  it("replays existing lines then reads only appended bytes", async () => {
    writeFileSync(file, "one\ntwo\n");
    const { tailer, lines } = collect();
    await tailer.poll();
    expect(lines).toEqual(["one", "two"]);
    appendFileSync(file, "three\n");
    await tailer.poll();
    expect(lines).toEqual(["one", "two", "three"]);
    expect(tailer.position).toBe(14);
  });

  it("holds a partial line across polls", async () => {
    writeFileSync(file, "done\npart");
    const { tailer, lines } = collect();
    await tailer.poll();
    expect(lines).toEqual(["done"]);
    appendFileSync(file, "ial\n");
    await tailer.poll();
    expect(lines).toEqual(["done", "partial"]);
  });

  it("restarts from zero when the file is truncated", async () => {
    writeFileSync(file, "aaaa\nbbbb\n");
    const { tailer, lines, resets } = collect();
    await tailer.poll();
    truncateSync(file, 0);
    appendFileSync(file, "c\n");
    await tailer.poll();
    expect(resets()).toBe(1);
    expect(lines).toEqual(["aaaa", "bbbb", "c"]);
  });

  it("restarts from zero when the file is replaced", async () => {
    writeFileSync(file, "old\n");
    const { tailer, lines, resets } = collect();
    await tailer.poll();
    const next = path.join(dir, "next.jsonl");
    writeFileSync(next, "new-first\nnew-second\n");
    renameSync(next, file);
    await tailer.poll();
    expect(resets()).toBe(1);
    expect(lines).toEqual(["old", "new-first", "new-second"]);
  });

  it("delivers appends through start() and stops cleanly", async () => {
    writeFileSync(file, "x\n");
    const lines: string[] = [];
    const tailer = new FileTailer(file, { onLines: (b) => lines.push(...b), pollMs: 20 });
    await tailer.start();
    expect(lines).toEqual(["x"]);
    appendFileSync(file, "y\n");
    await expect.poll(() => lines, { timeout: 2000 }).toEqual(["x", "y"]);
    tailer.stop();
    tailer.stop();
    appendFileSync(file, "z\n");
    await new Promise((r) => setTimeout(r, 60));
    expect(lines).toEqual(["x", "y"]);
  });
});
