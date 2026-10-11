/** The chain of incumbents, from the baseline to the current one. */

import { formatMetric, shortId } from "@/lib/format";
import type { RunState } from "@/lib/reducer";

export function Lineage({ state }: { state: RunState }) {
  if (state.lineage.length === 0) return <p className="notice">No incumbent yet.</p>;
  const metric = state.goal?.metric ?? "metric";
  return (
    <ol className="lineage" aria-label="Incumbent lineage">
      {state.lineage.map((entry, i) => {
        const current = i === state.lineage.length - 1;
        return (
          <li key={`${entry.id}-${i}`}>
            {i > 0 ? (
              <span className="arrow" aria-hidden="true">
                →
              </span>
            ) : null}
            <span className={`node${current ? " current" : ""}`} title={entry.id}>
              <span className="mono">{shortId(entry.id)}</span>
              <small>
                {i === 0 ? "baseline" : `epoch ${entry.epoch}`} · {metric} {formatMetric(entry.metric)}
              </small>
            </span>
          </li>
        );
      })}
    </ol>
  );
}
