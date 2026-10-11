/**
 * Maps file-access failures to HTTP responses for the route handlers.
 */

import { PathRejected } from "./paths";
import { SnapshotTooLarge } from "./snapshot";

/** 400 for a rejected path, 404 for a missing one, 413 for an oversized snapshot, 500 otherwise. */
export function errorResponse(error: unknown): Response {
  if (error instanceof PathRejected) {
    return Response.json({ error: error.message }, { status: 400 });
  }
  if (error instanceof SnapshotTooLarge) {
    return Response.json({ error: error.message }, { status: 413 });
  }
  const code = (error as { code?: unknown } | null)?.code;
  if (code === "ENOENT" || code === "ENOTDIR") {
    return Response.json({ error: "not found" }, { status: 404 });
  }
  return Response.json({ error: "internal error" }, { status: 500 });
}

/** Decodes a dynamic route segment that may still be percent-encoded. */
export function routeParam(value: string): string {
  if (!value.includes("%")) return value;
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}
