#!/usr/bin/env node
// Replays the fixture trace into a fresh run directory with delays, so the
// visualizer's live tailing can be demoed without building the Rust loop.
//
// Usage: node scripts/simulate-run.mjs [--out <dir>] [--delay <ms>] [--source <events.jsonl>]
//
// Defaults: source fixtures/curve-fit/events.jsonl, out fixtures/live-<timestamp>,
// delay 700 ms between lines. Some lines are written in two halves to exercise
// the reader's partial-line buffering. `elapsed_ms` is rewritten to wall time.

import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const app = path.resolve(here, "..");

function arg(name, fallback) {
  const index = process.argv.indexOf(`--${name}`);
  return index >= 0 && process.argv[index + 1] ? process.argv[index + 1] : fallback;
}

const source = path.resolve(arg("source", path.join(app, "fixtures/curve-fit/events.jsonl")));
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const out = path.resolve(arg("out", path.join(app, "fixtures", `live-${stamp}`)));
const delay = Number(arg("delay", "700"));

const lines = readFileSync(source, "utf8").split("\n").filter(Boolean);
mkdirSync(out, { recursive: true });
const file = path.join(out, "events.jsonl");
writeFileSync(file, "");
console.log(`writing ${lines.length} events to ${file}`);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const started = Date.now();

for (const [index, line] of lines.entries()) {
  const envelope = JSON.parse(line);
  envelope.elapsed_ms = Date.now() - started;
  const text = `${JSON.stringify(envelope)}\n`;
  if (index % 4 === 2) {
    const half = Math.floor(text.length / 2);
    appendFileSync(file, text.slice(0, half));
    await sleep(Math.min(250, delay / 2));
    appendFileSync(file, text.slice(half));
  } else {
    appendFileSync(file, text);
  }
  process.stdout.write(`${envelope.seq} ${envelope.event.kind}\n`);
  await sleep(delay);
}
console.log("done");
