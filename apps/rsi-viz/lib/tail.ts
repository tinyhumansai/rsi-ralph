/**
 * Tails an append-only file by byte offset.
 *
 * The tailer never writes or locks the file. Each `poll()` reads whatever was
 * appended since the last read and hands complete lines to `onLines`. If the
 * file shrinks or is replaced (a different inode), it calls `onReset` and
 * starts over from offset 0. `start()` drives `poll()` from `fs.watch` plus an
 * interval fallback, because `fs.watch` is unreliable on some filesystems.
 */

import { promises as fsp, watch, type FSWatcher } from "node:fs";
import path from "node:path";
import { LineSplitter } from "./lines";

export interface TailerOptions {
  /** Called with each batch of complete lines, in file order. */
  onLines: (lines: string[]) => void;
  /** Called when the file was truncated or recreated, before re-reading it. */
  onReset?: () => void;
  /** Poll interval in milliseconds for the fallback timer. */
  pollMs?: number;
  /** Largest read per poll, to bound memory on a huge backlog. */
  chunkBytes?: number;
}

export class FileTailer {
  private readonly splitter = new LineSplitter();
  private offset = 0;
  private ino: number | null = null;
  private watcher: FSWatcher | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  private running: Promise<void> | null = null;
  private again = false;
  private stopped = false;

  constructor(
    private readonly file: string,
    private readonly options: TailerOptions,
  ) {}

  /** Current read offset in bytes (complete and buffered bytes consumed). */
  get position(): number {
    return this.offset;
  }

  /**
   * Reads everything new since the last call. Concurrent calls coalesce: a
   * call made while a read is in flight schedules one more pass.
   */
  poll(): Promise<void> {
    if (this.running) {
      this.again = true;
      return this.running;
    }
    this.running = (async () => {
      do {
        this.again = false;
        await this.readOnce();
      } while (this.again && !this.stopped);
    })().finally(() => {
      this.running = null;
    });
    return this.running;
  }

  /** Starts watching; the first poll replays the existing contents. */
  start(): Promise<void> {
    const pollMs = this.options.pollMs ?? 500;
    this.timer = setInterval(() => void this.poll(), pollMs);
    try {
      // Watch the directory, not the file, so a recreated file is still seen.
      const base = path.basename(this.file);
      this.watcher = watch(path.dirname(this.file), (_event, name) => {
        if (!name || name.toString() === base) void this.poll();
      });
      this.watcher.on("error", () => this.closeWatcher());
    } catch {
      this.watcher = null;
    }
    return this.poll();
  }

  /** Stops the watcher and timer. Safe to call more than once. */
  stop(): void {
    this.stopped = true;
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.closeWatcher();
  }

  private closeWatcher(): void {
    this.watcher?.close();
    this.watcher = null;
  }

  private reset(): void {
    this.offset = 0;
    this.splitter.reset();
    this.options.onReset?.();
  }

  private async readOnce(): Promise<void> {
    let handle: fsp.FileHandle;
    try {
      handle = await fsp.open(this.file, "r");
    } catch {
      return; // Not created yet, or briefly missing while being replaced.
    }
    try {
      const stat = await handle.stat();
      if (this.ino !== null && stat.ino !== this.ino) this.reset();
      else if (stat.size < this.offset) this.reset();
      this.ino = stat.ino;
      const chunkBytes = this.options.chunkBytes ?? 1 << 20;
      while (!this.stopped && this.offset < stat.size) {
        const length = Math.min(chunkBytes, stat.size - this.offset);
        const buffer = new Uint8Array(length);
        const { bytesRead } = await handle.read(buffer, 0, length, this.offset);
        if (bytesRead === 0) break;
        this.offset += bytesRead;
        const lines = this.splitter.push(buffer.subarray(0, bytesRead));
        if (lines.length > 0) this.options.onLines(lines);
      }
    } finally {
      await handle.close();
    }
  }
}
