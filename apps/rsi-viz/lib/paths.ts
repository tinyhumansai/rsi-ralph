/**
 * Path guard for run ids and snapshot file names taken from URLs.
 *
 * Every file the server opens goes through `resolveInRoot`: the request is
 * validated lexically (no `..`, no absolute path, no NUL, no backslash), then
 * resolved with `realpath` and required to stay inside the real root, so a
 * symlink inside the root cannot point the server elsewhere.
 */

import { promises as fsp } from "node:fs";
import path from "node:path";

/** Raised for any request that names a path outside the run root. */
export class PathRejected extends Error {
  constructor(reason: string) {
    super(reason);
    this.name = "PathRejected";
  }
}

/** Snapshot files the server is willing to return. */
export const SNAPSHOT_FILES = [
  "archive.json",
  "epoch.json",
  "experiment-report.json",
  "ledger.json",
  "deployment.json",
] as const;

const GENERATION_FILE = /^[A-Za-z0-9_-]+\.generation-\d+\.json$/;

/** Whether `name` is an allowlisted snapshot file name (a bare name). */
export function isSnapshotFile(name: string): boolean {
  return (SNAPSHOT_FILES as readonly string[]).includes(name) || GENERATION_FILE.test(name);
}

/**
 * Lexically validates a run id: a relative, `/`-separated path whose segments
 * are non-empty and never `.` or `..`. `"."` (or `""`) names the root itself.
 */
export function validateRunId(id: string): string[] {
  if (id.includes("\0")) throw new PathRejected("run id contains NUL");
  if (id.includes("\\")) throw new PathRejected("run id contains a backslash");
  if (id === "" || id === ".") return [];
  if (id.startsWith("/") || path.isAbsolute(id) || /^[A-Za-z]:/.test(id)) {
    throw new PathRejected("run id is absolute");
  }
  const segments = id.split("/");
  for (const segment of segments) {
    if (segment === "" || segment === "." || segment === "..") {
      throw new PathRejected("run id has an empty, '.' or '..' segment");
    }
  }
  return segments;
}

function isInside(root: string, candidate: string): boolean {
  const rel = path.relative(root, candidate);
  return rel === "" || (rel !== ".." && !rel.startsWith(`..${path.sep}`) && !path.isAbsolute(rel));
}

/**
 * Resolves `runId` (and optionally a file inside it) under `root`, following
 * symlinks, and throws `PathRejected` unless the real path stays inside the
 * real root. Throws the underlying `ENOENT` error when the path is missing.
 */
export async function resolveInRoot(root: string, runId: string, file?: string): Promise<string> {
  const segments = validateRunId(runId);
  if (file !== undefined) {
    if (file.includes("\0") || file.includes("/") || file.includes("\\") || file === "." || file === "..") {
      throw new PathRejected("file name is not a bare name");
    }
    segments.push(file);
  }
  const realRoot = await fsp.realpath(root);
  const joined = path.join(realRoot, ...segments);
  if (!isInside(realRoot, joined)) throw new PathRejected("path escapes the run root");
  const real = await fsp.realpath(joined);
  if (!isInside(realRoot, real)) throw new PathRejected("path escapes the run root");
  return real;
}

/** The configured run root: `RSI_RUN_ROOT`, or the workspace `target/`. */
export function runRoot(): string {
  const configured = process.env.RSI_RUN_ROOT;
  // The root is chosen at run time and is never part of the build output, so
  // keep the bundler from tracing it into the server bundle.
  return path.resolve(/*turbopackIgnore: true*/ process.cwd(), configured && configured.length > 0 ? configured : "../../target");
}

/** Encodes a run id for a URL path segment. */
export function encodeRunId(id: string): string {
  return encodeURIComponent(id);
}
