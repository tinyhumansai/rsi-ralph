"use client";

/**
 * Live view of one trace run. Subscribes to the run's SSE endpoint, folds
 * every batch into `ViewState`, and renders the header, loop ring, metric
 * chart, lineage, and proposals feed.
 */

import { useEffect, useReducer, useState } from "react";
import { formatDuration, formatMetric, shortId } from "@/lib/format";
import { initialViewState, runStatus, viewReducer } from "@/lib/view";
import { Lineage } from "./Lineage";
import { LoopRing } from "./LoopRing";
import { MetricChart } from "./MetricChart";
import { ProposalsFeed } from "./ProposalsFeed";

const REASON_LABEL = {
  goal_reached: "goal reached",
  max_epochs: "max epochs",
  stalled: "stalled",
  budget: "budget spent",
  error: "error",
  provider_refused: "provider refused",
} as const;

export function TraceView({ runId }: { runId: string }) {
  const [view, dispatch] = useReducer(viewReducer, undefined, initialViewState);
  const [connected, setConnected] = useState(true);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const source = new EventSource(`/api/runs/${encodeURIComponent(runId)}/events`);
    source.onopen = () => setConnected(true);
    source.onerror = () => setConnected(source.readyState === EventSource.OPEN);
    source.onmessage = (message: MessageEvent<string>) => {
      dispatch({ type: "batch", data: message.data, now: Date.now() });
    };
    source.addEventListener("reset", () => dispatch({ type: "reset" }));
    source.addEventListener("ready", (message) => {
      let mtimeMs: number | null = null;
      try {
        const parsed = JSON.parse((message as MessageEvent<string>).data) as { mtimeMs?: unknown };
        mtimeMs = typeof parsed.mtimeMs === "number" ? parsed.mtimeMs : null;
      } catch {
        mtimeMs = null;
      }
      dispatch({ type: "ready", mtimeMs });
    });
    return () => source.close();
  }, [runId]);

  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 5000);
    return () => clearInterval(timer);
  }, []);

  const run = view.run;
  const status = runStatus(view, now);
  const goal = run.goal;
  const live = status === "live";

  return (
    <div className="stack">
      <section>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "4px 12px", alignItems: "center" }}>
          <h1 className="mono">{run.runId ?? runId}</h1>
          {status === "finished" && run.finished ? (
            <span className="badge">finished · {REASON_LABEL[run.finished.reason]}</span>
          ) : (
            <span className={`badge${live ? " live" : ""}`}>{status}</span>
          )}
          {!connected && status !== "finished" ? <span className="badge reject">reconnecting</span> : null}
        </div>
        {run.subject ? <p className="secondary" style={{ margin: 0 }}>subject <code>{run.subject}</code></p> : null}
        <dl className="facts">
          <div>
            <dt>Goal</dt>
            <dd>
              {goal
                ? `${goal.direction} ${goal.metric}${goal.target !== null ? ` to ${formatMetric(goal.target)}` : ""}`
                : "—"}
            </dd>
          </div>
          <div>
            <dt>Incumbent</dt>
            <dd>
              <span className="mono" title={run.incumbent ?? undefined}>
                {run.incumbent ? shortId(run.incumbent, 12) : "—"}
              </span>{" "}
              · {goal?.metric ?? "metric"} {formatMetric(run.incumbentMetric)}
            </dd>
          </div>
          <div>
            <dt>Epoch</dt>
            <dd>{run.epoch ?? "—"}</dd>
          </div>
          <div>
            <dt>Elapsed</dt>
            <dd>{formatDuration(run.elapsedMs)}</dd>
          </div>
          <div>
            <dt>Events</dt>
            <dd>{run.lastSeq + 1}</dd>
          </div>
        </dl>
        {view.invalid > 0 || run.gaps > 0 ? (
          <p className="notice" style={{ marginTop: 12 }}>
            {view.invalid > 0 ? `${view.invalid} unreadable line(s) skipped (last: ${view.lastError}). ` : ""}
            {run.gaps > 0 ? `${run.gaps} sequence number(s) missing from the trace.` : ""}
          </p>
        ) : null}
      </section>

      <div className="grid two">
        <section className="card">
          <h2>Loop</h2>
          <LoopRing stage={run.stage} live={live} epoch={run.epoch} />
        </section>
        <section className="card">
          <h2>{goal?.metric ?? "Metric"} by epoch</h2>
          <MetricChart state={run} />
        </section>
      </div>

      <section className="card">
        <h2>Incumbent lineage</h2>
        <Lineage state={run} />
      </section>

      <section className="card">
        <h2>Proposals</h2>
        <ProposalsFeed state={run} />
      </section>
    </div>
  );
}
