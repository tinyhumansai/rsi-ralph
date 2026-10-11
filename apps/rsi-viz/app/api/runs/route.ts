import { discoverRuns } from "@/lib/runs";
import { runRoot } from "@/lib/paths";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/** Lists trace and snapshot runs under `RSI_RUN_ROOT`, newest first. */
export async function GET(): Promise<Response> {
  return Response.json(await discoverRuns(runRoot()), { headers: { "Cache-Control": "no-store" } });
}
