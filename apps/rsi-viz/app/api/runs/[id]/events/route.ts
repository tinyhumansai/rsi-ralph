import { errorResponse, routeParam } from "@/lib/http";
import { resolveInRoot, runRoot } from "@/lib/paths";
import { createEventStream } from "@/lib/sse";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

/** Streams a run's `events.jsonl` as Server-Sent Events: replay, then tail. */
export async function GET(request: Request, { params }: { params: Promise<{ id: string }> }): Promise<Response> {
  const { id } = await params;
  let file: string;
  try {
    file = await resolveInRoot(runRoot(), routeParam(id), "events.jsonl");
  } catch (error) {
    return errorResponse(error);
  }
  return new Response(createEventStream(file, request.signal), {
    headers: {
      "Content-Type": "text/event-stream; charset=utf-8",
      "Cache-Control": "no-cache, no-transform",
      Connection: "keep-alive",
      "X-Accel-Buffering": "no",
    },
  });
}
