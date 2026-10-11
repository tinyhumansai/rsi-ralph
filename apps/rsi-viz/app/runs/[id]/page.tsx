import { notFound } from "next/navigation";
import { JsonView } from "@/components/JsonView";
import { ReportView } from "@/components/ReportView";
import { TraceView } from "@/components/TraceView";
import { routeParam } from "@/lib/http";
import { encodeRunId, runRoot } from "@/lib/paths";
import { asExperimentReport } from "@/lib/report";
import { discoverRuns } from "@/lib/runs";
import { readSnapshot } from "@/lib/snapshot";

export const dynamic = "force-dynamic";

async function loadSnapshot(root: string, id: string, file: string): Promise<{ text: string } | { error: string }> {
  try {
    return { text: await readSnapshot(root, id, file) };
  } catch (error) {
    return { error: error instanceof Error ? error.message : "unreadable" };
  }
}

export default async function RunPage({ params }: { params: Promise<{ id: string }> }) {
  const id = routeParam((await params).id);
  const root = runRoot();
  let runs;
  try {
    runs = await discoverRuns(root);
  } catch {
    notFound();
  }
  const run = runs.find((r) => r.id === id);
  if (!run) notFound();
  const snapshots = run.files.filter((f) => f !== "events.jsonl");

  const sections = await Promise.all(
    snapshots.map(async (file) => {
      const loaded = await loadSnapshot(root, id, file);
      if ("error" in loaded) return { file, error: loaded.error };
      let parsed: unknown = undefined;
      try {
        parsed = JSON.parse(loaded.text);
      } catch {
        return { file, error: "invalid JSON" };
      }
      return { file, parsed };
    }),
  );

  return (
    <div className="stack">
      {run.kind === "trace" ? (
        <TraceView runId={id} />
      ) : (
        <div>
          <h1 className="mono">{id}</h1>
          <p className="muted" style={{ margin: 0 }}>
            Snapshot-only run · updated {new Date(run.updatedAt).toLocaleString()}
          </p>
        </div>
      )}
      {sections.map((section) => {
        const report = section.file === "experiment-report.json" ? asExperimentReport(section.parsed) : null;
        const href = `/api/runs/${encodeRunId(id)}/snapshot/${encodeURIComponent(section.file)}`;
        return (
          <section key={section.file} className="card">
            <h2>
              <a href={href} className="mono">
                {section.file}
              </a>
            </h2>
            {"error" in section ? (
              <p className="notice">Could not read this file: {section.error}</p>
            ) : report ? (
              <>
                <ReportView report={report} />
                <details style={{ marginTop: 12 }}>
                  <summary className="secondary">Raw JSON</summary>
                  <JsonView value={section.parsed} />
                </details>
              </>
            ) : (
              <JsonView value={section.parsed} />
            )}
          </section>
        );
      })}
    </div>
  );
}
