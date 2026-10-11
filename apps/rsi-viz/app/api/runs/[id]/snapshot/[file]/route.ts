import { readSnapshot } from "@/lib/snapshot";
import { errorResponse, routeParam } from "@/lib/http";
import { runRoot } from "@/lib/paths";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/** Returns one allowlisted snapshot JSON file from a run directory. */
export async function GET(
  _request: Request,
  { params }: { params: Promise<{ id: string; file: string }> },
): Promise<Response> {
  const { id, file } = await params;
  try {
    const text = await readSnapshot(runRoot(), routeParam(id), routeParam(file));
    return new Response(text, {
      headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" },
    });
  } catch (error) {
    return errorResponse(error);
  }
}
