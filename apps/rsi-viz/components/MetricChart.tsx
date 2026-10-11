/**
 * Metric against epoch: the incumbent as a step line, every measured
 * candidate as a point coloured and shaped by its decision, and the goal
 * target as a dashed line. Tooltips are native SVG `<title>` elements.
 */

import type { Decision, Goal } from "@/lib/events";
import { formatMetric, formatSigned, niceTicks, shortId } from "@/lib/format";
import type { Candidate, RunState } from "@/lib/reducer";

const WIDTH = 640;
const HEIGHT = 300;
const M = { top: 16, right: 16, bottom: 40, left: 56 };

export const DECISION_COLOR: Record<Decision | "pending", string> = {
  accept: "var(--accept)",
  reject: "var(--reject)",
  inconclusive: "var(--inconclusive)",
  pending: "var(--pending)",
};

export function DecisionMarker({ decision, x, y }: { decision: Decision | null; x: number; y: number }) {
  const color = DECISION_COLOR[decision ?? "pending"];
  if (decision === "reject") {
    const d = 4.5;
    return (
      <path
        d={`M${x - d},${y - d} L${x + d},${y + d} M${x - d},${y + d} L${x + d},${y - d}`}
        stroke={color}
        strokeWidth={2.5}
        strokeLinecap="round"
      />
    );
  }
  if (decision === "inconclusive") {
    const d = 5.5;
    return (
      <path
        d={`M${x},${y - d} L${x + d},${y} L${x},${y + d} L${x - d},${y} z`}
        fill={color}
        stroke="var(--surface)"
        strokeWidth={1.5}
      />
    );
  }
  return (
    <circle
      cx={x}
      cy={y}
      r={decision === null ? 4 : 5}
      fill={decision === null ? "var(--surface)" : color}
      stroke={decision === null ? color : "var(--surface)"}
      strokeWidth={decision === null ? 2 : 1.5}
    />
  );
}

function candidateTip(c: Candidate, goal: Goal | null): string {
  const metric = goal?.metric ?? "metric";
  const parts = [`epoch ${c.epoch} · ${c.id}`, `${metric} ${formatMetric(c.mean)}`];
  if (c.proposalId) parts.push(`proposal ${c.proposalId}`);
  if (c.decision) {
    const units = c.units === "quality" ? "quality " : "";
    parts.push(`${c.decision}: ${units}${formatSigned(c.estimate)} [${formatMetric(c.lower)}, ${formatMetric(c.upper)}]`);
  } else if (c.epoch === 0) {
    parts.push("baseline");
  }
  if (c.failures > 0) parts.push(`${c.failures}/${c.seeds} seeds failed`);
  return parts.join("\n");
}

export function MetricChart({ state }: { state: RunState }) {
  const goal = state.goal;
  const points = state.candidateOrder
    .map((id) => state.candidates[id]!)
    .filter((c): c is Candidate & { mean: number } => c.mean !== null);
  const series = state.series.filter((p): p is { epoch: number; metric: number } => p.metric !== null);
  const values = [...points.map((p) => p.mean), ...series.map((p) => p.metric)];
  if (goal?.target != null) values.push(goal.target);
  if (values.length === 0) {
    return <p className="notice">No measurements yet.</p>;
  }
  const maxEpoch = Math.max(1, state.epoch ?? 0, ...points.map((p) => p.epoch), ...series.map((p) => p.epoch));
  const yTicks = niceTicks(Math.min(...values), Math.max(...values), 5);
  const y0 = yTicks[0]!;
  const y1 = yTicks[yTicks.length - 1]!;
  const plotW = WIDTH - M.left - M.right;
  const plotH = HEIGHT - M.top - M.bottom;
  // Pad the epoch domain so points on the first and last epoch are not clipped.
  const pad = 0.25;
  const x = (epoch: number) => M.left + ((epoch + pad) / (maxEpoch + 2 * pad)) * plotW;
  const y = (v: number) => M.top + (1 - (v - y0) / (y1 - y0 || 1)) * plotH;
  const xTicks = niceTicks(0, maxEpoch, Math.min(maxEpoch, 8)).filter((t) => Number.isInteger(t) && t <= maxEpoch);

  let step = "";
  series.forEach((p, i) => {
    if (i === 0) step = `M${x(p.epoch)},${y(p.metric)}`;
    else step += ` H${x(p.epoch)} V${y(p.metric)}`;
  });
  const last = series[series.length - 1];
  if (last && !state.finished && last.epoch < maxEpoch) step += ` H${x(maxEpoch)}`;

  // Spread points that share an epoch so they do not sit on top of each other.
  const perEpoch = new Map<number, number>();
  const placed = points.map((c) => {
    const index = perEpoch.get(c.epoch) ?? 0;
    perEpoch.set(c.epoch, index + 1);
    return { c, index };
  });
  const jitter = (c: Candidate, index: number) => {
    const count = perEpoch.get(c.epoch) ?? 1;
    const spread = Math.min(14, plotW / (maxEpoch + 1) / 4);
    return count === 1 ? 0 : (index - (count - 1) / 2) * (spread / Math.max(1, count - 1)) * 2;
  };

  const directionNote = goal ? (goal.direction === "minimize" ? "lower is better" : "higher is better") : "";
  return (
    <figure style={{ margin: 0 }}>
      <svg
        className="chart scaled"
        viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
        role="img"
        aria-label={`${goal?.metric ?? "metric"} by epoch`}
      >
        {yTicks.map((t) => (
          <g key={`y-${t}`}>
            <line x1={M.left} x2={WIDTH - M.right} y1={y(t)} y2={y(t)} stroke="var(--grid)" />
            <text x={M.left - 8} y={y(t) + 4} textAnchor="end" style={{ fontVariantNumeric: "tabular-nums" }}>
              {formatMetric(t)}
            </text>
          </g>
        ))}
        <line x1={M.left} x2={WIDTH - M.right} y1={M.top + plotH} y2={M.top + plotH} stroke="var(--axis)" />
        {xTicks.map((t) => (
          <text key={`x-${t}`} x={x(t)} y={M.top + plotH + 16} textAnchor="middle">
            {t}
          </text>
        ))}
        <text x={M.left + plotW / 2} y={HEIGHT - 4} textAnchor="middle">
          epoch
        </text>
        <text transform={`translate(12 ${M.top + plotH / 2}) rotate(-90)`} textAnchor="middle">
          {goal?.metric ?? "metric"}
          {directionNote ? ` (${directionNote})` : ""}
        </text>
        {goal?.target != null ? (
          <g>
            <title>{`target ${formatMetric(goal.target)}`}</title>
            <line
              x1={M.left}
              x2={WIDTH - M.right}
              y1={y(goal.target)}
              y2={y(goal.target)}
              stroke="var(--ink-2)"
              strokeDasharray="5 4"
              strokeWidth={1.5}
            />
            <text x={M.left + 6} y={y(goal.target) - 6} textAnchor="start" style={{ fill: "var(--ink-2)" }}>
              target {formatMetric(goal.target)}
            </text>
          </g>
        ) : null}
        {step ? (
          <path d={step} fill="none" stroke="var(--series)" strokeWidth={2} strokeLinejoin="round" />
        ) : null}
        {series.map((p) => (
          <g key={`s-${p.epoch}`}>
            <title>{`incumbent after epoch ${p.epoch}: ${formatMetric(p.metric)}`}</title>
            <circle cx={x(p.epoch)} cy={y(p.metric)} r={10} fill="transparent" />
          </g>
        ))}
        {placed.map(({ c, index }) => {
          const cx = x(c.epoch) + jitter(c, index);
          const cy = y(c.mean);
          return (
            <g key={c.id}>
              <title>{candidateTip(c, goal)}</title>
              <circle cx={cx} cy={cy} r={11} fill="transparent" />
              {c.epoch === 0 ? (
                <circle cx={cx} cy={cy} r={5} fill="var(--series)" stroke="var(--surface)" strokeWidth={1.5} />
              ) : (
                <DecisionMarker decision={c.decision} x={cx} y={cy} />
              )}
            </g>
          );
        })}
        {last ? (
          <text
            x={x(last.epoch) + (x(last.epoch) > WIDTH - 90 ? -8 : 8)}
            y={y(last.metric) + 16}
            textAnchor={x(last.epoch) > WIDTH - 90 ? "end" : "start"}
            style={{ fill: "var(--series)" }}
          >
            {shortId(state.incumbent ?? "")}
          </text>
        ) : null}
      </svg>
      <figcaption className="legend">
        <span>
          <svg width="22" height="10" aria-hidden="true">
            <line x1="1" x2="21" y1="5" y2="5" stroke="var(--series)" strokeWidth="2" />
          </svg>
          incumbent
        </span>
        {(["accept", "reject", "inconclusive"] as const).map((d) => (
          <span key={d}>
            <svg width="14" height="14" aria-hidden="true">
              <DecisionMarker decision={d} x={7} y={7} />
            </svg>
            {d === "accept" ? "accepted" : d === "reject" ? "rejected" : "inconclusive"}
          </span>
        ))}
        <span>
          <svg width="14" height="14" aria-hidden="true">
            <DecisionMarker decision={null} x={7} y={7} />
          </svg>
          awaiting score
        </span>
        {goal?.target != null ? (
          <span>
            <svg width="22" height="10" aria-hidden="true">
              <line x1="1" x2="21" y1="5" y2="5" stroke="var(--ink-2)" strokeDasharray="5 4" strokeWidth="1.5" />
            </svg>
            target
          </span>
        ) : null}
      </figcaption>
    </figure>
  );
}
