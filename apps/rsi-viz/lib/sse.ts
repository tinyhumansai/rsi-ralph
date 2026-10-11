/**
 * Server-Sent Events stream over a tailed `events.jsonl`.
 *
 * Each batch of complete lines becomes one SSE message with one `data:` field
 * per line, so the browser receives the batch as a single `\n`-joined string.
 * Other messages:
 *
 * - `event: reset` when the file was truncated or recreated; the client must
 *   discard its state.
 * - `event: ready` once the existing contents were replayed, carrying the
 *   file's modification time as `{ "mtimeMs": number }`.
 * - `: ping` comment lines as a heartbeat, so idle proxies keep the
 *   connection open.
 *
 * The stream stops its tailer and timers when the request is aborted or the
 * reader cancels.
 */

import { promises as fsp } from "node:fs";
import { FileTailer } from "./tail";

export interface EventStreamOptions {
  heartbeatMs?: number;
  pollMs?: number;
}

/** Formats lines as one SSE message. Lines must not contain `\n`. */
export function formatLines(lines: string[]): string {
  return `${lines.map((line) => `data: ${line}`).join("\n")}\n\n`;
}

/** Formats a named SSE event with a JSON payload. */
export function formatEvent(name: string, payload: unknown): string {
  return `event: ${name}\ndata: ${JSON.stringify(payload)}\n\n`;
}

export function createEventStream(
  file: string,
  signal: AbortSignal,
  options: EventStreamOptions = {},
): ReadableStream<Uint8Array> {
  const encoder = new TextEncoder();
  let close = () => {};
  return new ReadableStream<Uint8Array>({
    async start(controller) {
      let closed = false;
      const send = (text: string) => {
        if (closed) return;
        try {
          controller.enqueue(encoder.encode(text));
        } catch {
          close();
        }
      };
      const tailer = new FileTailer(file, {
        onLines: (lines) => send(formatLines(lines)),
        onReset: () => send(formatEvent("reset", {})),
        pollMs: options.pollMs,
      });
      const heartbeat = setInterval(() => send(": ping\n\n"), options.heartbeatMs ?? 15_000);
      close = () => {
        if (closed) return;
        closed = true;
        tailer.stop();
        clearInterval(heartbeat);
        signal.removeEventListener("abort", close);
        try {
          controller.close();
        } catch {
          // Already closed by the runtime.
        }
      };
      if (signal.aborted) {
        close();
        return;
      }
      signal.addEventListener("abort", close);
      send("retry: 2000\n\n");
      await tailer.start();
      let mtimeMs: number | null = null;
      try {
        mtimeMs = (await fsp.stat(file)).mtimeMs;
      } catch {
        mtimeMs = null;
      }
      send(formatEvent("ready", { mtimeMs }));
    },
    cancel() {
      close();
    },
  });
}
