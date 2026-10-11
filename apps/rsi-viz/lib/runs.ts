/**
 * Run discovery under the run root.
 *
 * A directory holding `events.jsonl` is a trace run. Any other directory
 * holding a hive snapshot (`archive.json`, `epoch.json`,
 * `experiment-report.json`) is a snapshot-only run. The walk is bounded in
 * depth, skips dot-directories and `node_modules`, and does not follow
 * symlinked directories, so a large `target/` stays cheap to scan.
 */

import { promises as fsp, type Dirent } from "node:fs";
import path from "node:path";
import { isSnapshotFile } from "./paths";

export type RunKind = "trace" | "snapshot";

export interface RunSummary {
  /** Directory relative to the root with `/` separators; `"."` is the root. */
  id: string;
  kind: RunKind;
  /** ISO time of the newest listed file's modification. */
  updatedAt: string;
  /** `events.jsonl` and allowlisted snapshot files present in the directory. */
  files: string[];
}

const EVENTS_FILE = "events.jsonl";
const SNAPSHOT_MARKERS = new Set(["archive.json", "epoch.json", "experiment-report.json"]);
/** Cargo build output directories that never hold runs and can be huge. */
const SKIP = new Set(["node_modules", "debug", "release", "deps", "build", "incremental", "doc"]);

async function readDir(dir: string): Promise<Dirent[]> {
  try {
    return await fsp.readdir(dir, { withFileTypes: true });
  } catch {
    return [];
  }
}

async function summarize(root: string, dir: string, entries: Dirent[]): Promise<RunSummary | null> {
  const names = entries.filter((e) => e.isFile()).map((e) => e.name);
  const hasEvents = names.includes(EVENTS_FILE);
  const hasSnapshot = names.some((n) => SNAPSHOT_MARKERS.has(n));
  if (!hasEvents && !hasSnapshot) return null;
  const files = names.filter((n) => n === EVENTS_FILE || isSnapshotFile(n)).sort();
  let newest = 0;
  for (const name of files) {
    try {
      newest = Math.max(newest, (await fsp.stat(path.join(dir, name))).mtimeMs);
    } catch {
      // The file vanished between readdir and stat; ignore it.
    }
  }
  const rel = path.relative(root, dir).split(path.sep).join("/");
  return {
    id: rel === "" ? "." : rel,
    kind: hasEvents ? "trace" : "snapshot",
    updatedAt: new Date(newest).toISOString(),
    files,
  };
}

/** Lists runs under `root` to `maxDepth` levels, newest first. */
export async function discoverRuns(root: string, maxDepth = 3): Promise<RunSummary[]> {
  const runs: RunSummary[] = [];
  const walk = async (dir: string, depth: number): Promise<void> => {
    const entries = await readDir(dir);
    const run = await summarize(root, dir, entries);
    if (run) runs.push(run);
    if (depth >= maxDepth) return;
    for (const entry of entries) {
      if (!entry.isDirectory() || entry.name.startsWith(".") || SKIP.has(entry.name)) continue;
      await walk(path.join(dir, entry.name), depth + 1);
    }
  };
  await walk(path.resolve(root), 0);
  runs.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt) || a.id.localeCompare(b.id));
  return runs;
}
