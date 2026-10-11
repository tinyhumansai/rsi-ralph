import { appendFileSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { createEventStream, formatEvent, formatLines } from "./sse";

let dir: string;
let file: string;

beforeEach(() => {
  dir = mkdtempSync(path.join(tmpdir(), "rsi-viz-sse-"));
  file = path.join(dir, "events.jsonl");
});

afterEach(() => rmSync(dir, { recursive: true, force: true }));

async function readUntil(reader: ReadableStreamDefaultReader<Uint8Array>, needle: string, sofar = ""): Promise<string> {
  const decoder = new TextDecoder();
  let text = sofar;
  while (!text.includes(needle)) {
    const { value, done } = await reader.read();
    if (done) break;
    text += decoder.decode(value);
  }
  return text;
}

describe("SSE formatting", () => {
  it("puts each line in its own data field", () => {
    expect(formatLines(["a", "b"])).toBe("data: a\ndata: b\n\n");
    expect(formatEvent("ready", { mtimeMs: 1 })).toBe('event: ready\ndata: {"mtimeMs":1}\n\n');
  });
});

describe("createEventStream", () => {
  it("replays, signals ready, tails appends, heartbeats, and closes on abort", async () => {
    writeFileSync(file, "one\ntwo\n");
    const abort = new AbortController();
    const reader = createEventStream(file, abort.signal, { pollMs: 20, heartbeatMs: 30 }).getReader();
    let text = await readUntil(reader, "event: ready");
    expect(text).toContain("retry: 2000\n\n");
    expect(text).toContain("data: one\ndata: two\n\n");
    appendFileSync(file, "three\n");
    text = await readUntil(reader, "data: three\n\n", text);
    text = await readUntil(reader, ": ping\n\n", text);
    abort.abort();
    expect(await reader.read()).toEqual({ value: undefined, done: true });
  });

  it("emits reset when the file shrinks", async () => {
    writeFileSync(file, "aaaa\nbbbb\n");
    const abort = new AbortController();
    const reader = createEventStream(file, abort.signal, { pollMs: 20 }).getReader();
    let text = await readUntil(reader, "event: ready");
    writeFileSync(file, "c\n");
    text = await readUntil(reader, "data: c\n\n", text);
    expect(text).toContain("event: reset\ndata: {}\n\n");
    await reader.cancel();
    abort.abort();
  });

  it("closes immediately for an already-aborted request", async () => {
    const abort = new AbortController();
    abort.abort();
    const reader = createEventStream(file, abort.signal).getReader();
    expect((await reader.read()).done).toBe(true);
  });
});
