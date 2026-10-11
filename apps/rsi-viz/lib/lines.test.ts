import { describe, expect, it } from "vitest";
import { LineSplitter } from "./lines";

const enc = (s: string) => new TextEncoder().encode(s);

describe("LineSplitter", () => {
  it("returns several complete lines from one chunk", () => {
    const s = new LineSplitter();
    expect(s.push(enc("a\nb\nc\n"))).toEqual(["a", "b", "c"]);
    expect(s.pendingBytes).toBe(0);
  });

  it("buffers a partial trailing line until its newline arrives", () => {
    const s = new LineSplitter();
    expect(s.push(enc('{"seq":0}\n{"se'))).toEqual(['{"seq":0}']);
    expect(s.pendingBytes).toBe(4);
    expect(s.push(enc('q":1'))).toEqual([]);
    expect(s.push(enc("}\n"))).toEqual(['{"seq":1}']);
    expect(s.pendingBytes).toBe(0);
  });

  it("keeps a multi-byte character split across chunks intact", () => {
    const s = new LineSplitter();
    const bytes = enc("héllo\n");
    expect(s.push(bytes.subarray(0, 2))).toEqual([]);
    expect(s.push(bytes.subarray(2))).toEqual(["héllo"]);
  });

  it("strips carriage returns and skips blank lines", () => {
    const s = new LineSplitter();
    expect(s.push(enc("a\r\n\n\r\nb\n"))).toEqual(["a", "b"]);
  });

  it("drops the buffered partial line on reset", () => {
    const s = new LineSplitter();
    s.push(enc("half"));
    s.reset();
    expect(s.push(enc("whole\n"))).toEqual(["whole"]);
  });
});
