/**
 * Splits a byte stream into complete `\n`-terminated lines.
 *
 * The trace writer flushes one line at a time, but a reader can still observe
 * a write half-way through. The splitter buffers the trailing partial line
 * (as bytes, so a multi-byte UTF-8 character split across reads survives)
 * until its newline arrives.
 */

const NEWLINE = 0x0a;
const decoder = new TextDecoder("utf-8");

export class LineSplitter {
  private pending: Uint8Array = new Uint8Array(0);

  /** Bytes buffered after the last complete line. */
  get pendingBytes(): number {
    return this.pending.length;
  }

  /**
   * Feeds a chunk and returns every line it completes, without the newline.
   * A trailing `\r` is stripped and blank lines are skipped.
   */
  push(chunk: Uint8Array): string[] {
    const data = this.pending.length === 0 ? chunk : concat(this.pending, chunk);
    const lines: string[] = [];
    let start = 0;
    for (let i = 0; i < data.length; i++) {
      if (data[i] !== NEWLINE) continue;
      let end = i;
      if (end > start && data[end - 1] === 0x0d) end -= 1;
      if (end > start) lines.push(decoder.decode(data.subarray(start, end)));
      start = i + 1;
    }
    this.pending = data.slice(start);
    return lines;
  }

  /** Drops any buffered partial line, e.g. after the file was truncated. */
  reset(): void {
    this.pending = new Uint8Array(0);
  }
}

function concat(a: Uint8Array, b: Uint8Array): Uint8Array {
  const out = new Uint8Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}
