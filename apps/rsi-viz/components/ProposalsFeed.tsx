/**
 * Proposals, newest first, each with its candidate's measurement and the
 * scored decision with the improvement estimate and its interval. A proposal
 * rejected before materialization (no candidate) is shown as not executed.
 */

import { formatMetric, formatSigned, shortId } from "@/lib/format";
import type { RunState } from "@/lib/reducer";

const DECISION_LABEL = {
  accept: "✓ accepted",
  reject: "✕ rejected",
  inconclusive: "◆ inconclusive",
} as const;

export function ProposalsFeed({ state }: { state: RunState }) {
  if (state.proposals.length === 0) {
    return <p className="notice">No proposals yet.</p>;
  }
  const metric = state.goal?.metric ?? "metric";
  return (
    <ol className="feed">
      {[...state.proposals].reverse().map((p) => {
        const c = p.candidate ? state.candidates[p.candidate] : undefined;
        const decision = c?.decision ?? null;
        return (
          <li key={`${p.epoch}-${p.proposalId}`} className={decision ?? "pending"}>
            <div className="head">
              <span className="muted">epoch {p.epoch}</span>
              <span className="mono">{p.proposalId}</span>
              {p.candidate ? (
                <span className="mono muted" title={p.candidate}>
                  → {shortId(p.candidate)}
                </span>
              ) : null}
              {decision ? (
                <span className={`badge ${decision}`}>{DECISION_LABEL[decision]}</span>
              ) : (
                <span className="badge pending">
                  {p.candidate === null ? "not executed" : c ? "measuring" : "awaiting execution"}
                </span>
              )}
            </div>
            <p>
              <span className="label">Rationale</span>
              {p.rationale}
            </p>
            <p>
              <span className="label">Hypothesis</span>
              {p.hypothesis}
            </p>
            {c && (c.mean !== null || c.seeds > 0) ? (
              <p className="secondary" style={{ fontVariantNumeric: "tabular-nums", fontSize: "0.85rem" }}>
                {metric} {formatMetric(c.mean)}
                {c.seeds > 0 ? ` over ${c.seeds} seed${c.seeds === 1 ? "" : "s"}` : ""}
                {c.failures > 0 ? `, ${c.failures} failed` : ""}
                {c.decision
                  ? ` · improvement ${formatSigned(c.estimate)} [${formatMetric(c.lower)}, ${formatMetric(c.upper)}]`
                  : ""}
              </p>
            ) : null}
          </li>
        );
      })}
    </ol>
  );
}
