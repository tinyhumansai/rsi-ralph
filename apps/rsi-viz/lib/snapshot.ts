/**
 * Reads allowlisted snapshot files from a run directory.
 */

import { promises as fsp } from "node:fs";
import { isSnapshotFile, PathRejected, resolveInRoot } from "./paths";

/** Largest snapshot the server will return, to keep the viewer responsive. */
export const MAX_SNAPSHOT_BYTES = 32 * 1024 * 1024;

/** Raised when a snapshot exceeds `MAX_SNAPSHOT_BYTES`. */
export class SnapshotTooLarge extends Error {
  constructor(bytes: number) {
    super(`snapshot is ${bytes} bytes, over the ${MAX_SNAPSHOT_BYTES} byte limit`);
    this.name = "SnapshotTooLarge";
  }
}

/**
 * Returns the text of `file` in run `runId`. Throws `PathRejected` for a file
 * outside the allowlist or root, and `ENOENT` for a missing one.
 */
export async function readSnapshot(root: string, runId: string, file: string): Promise<string> {
  if (!isSnapshotFile(file)) throw new PathRejected("file is not an allowlisted snapshot");
  const real = await resolveInRoot(root, runId, file);
  const stat = await fsp.stat(real);
  if (!stat.isFile()) throw new PathRejected("snapshot is not a regular file");
  if (stat.size > MAX_SNAPSHOT_BYTES) throw new SnapshotTooLarge(stat.size);
  return fsp.readFile(real, "utf8");
}

/** Whether `runId` holds an `events.jsonl` trace. */
export async function hasTrace(root: string, runId: string): Promise<boolean> {
  try {
    await resolveInRoot(root, runId, "events.jsonl");
    return true;
  } catch (error) {
    if (error instanceof PathRejected) throw error;
    return false;
  }
}
