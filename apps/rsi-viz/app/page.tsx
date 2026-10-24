import Link from "next/link";
import { encodeRunId, runRoot } from "@/lib/paths";
import { discoverRuns } from "@/lib/runs";

export const dynamic = "force-dynamic";

export default async function RunListPage() {
  const root = runRoot();
  const runs = await discoverRuns(root);
  return (
    <div className="stack run-list">
      <div>
        <h1>Runs</h1>
        <p className="muted mono" style={{ margin: 0, overflowWrap: "anywhere" }}>
          {root}
        </p>
      </div>
      {runs.length === 0 ? (
        <p className="notice">
          No runs found. Set <code>RSI_RUN_ROOT</code> to a directory holding <code>events.jsonl</code> traces or hive
          snapshots, or start a loop with <code>cargo run -p rsi-ralph --example script_loop</code>.
        </p>
      ) : (
        <ul>
          {runs.map((run) => (
            <li key={run.id}>
              <Link href={`/runs/${encodeRunId(run.id)}`}>
                <div className="card run-row">
                  <span className="id">{run.id}</span>
                  <span className="secondary">
                    <span className="badge">{run.kind === "trace" ? "trace" : "snapshot"}</span>{" "}
                    <time dateTime={run.updatedAt}>{new Date(run.updatedAt).toLocaleString()}</time>
                  </span>
                  <span className="muted" style={{ flexBasis: "100%", fontSize: "0.8rem" }}>
                    {run.files.join(" · ")}
                  </span>
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
