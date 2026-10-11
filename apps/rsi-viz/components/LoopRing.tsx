/**
 * The loop ring: the six stages in loop order around a circle, with the stage
 * of the latest event highlighted and pulsing while the run is live.
 */

import { STAGES, type Stage } from "@/lib/events";

const WIDTH = 340;
const SIZE = 260;
const CENTER_X = WIDTH / 2;
const CENTER = SIZE / 2;
const RADIUS = 88;
const NODE = 13;

function point(index: number, radius = RADIUS) {
  const angle = -Math.PI / 2 + (index / STAGES.length) * Math.PI * 2;
  return { x: CENTER_X + radius * Math.cos(angle), y: CENTER + radius * Math.sin(angle) };
}

export function LoopRing({ stage, live, epoch }: { stage: Stage | null; live: boolean; epoch: number | null }) {
  return (
    <svg
      className="chart"
      viewBox={`0 0 ${WIDTH} ${SIZE}`}
      role="img"
      aria-label={stage ? `Loop stage: ${stage}` : "Loop stage: not started"}
      style={{ maxWidth: 360, margin: "0 auto" }}
    >
      <defs>
        <marker id="ring-arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto">
          <path d="M0,0 L10,5 L0,10 z" fill="var(--axis)" />
        </marker>
      </defs>
      {STAGES.map((_, i) => {
        const gap = 0.22;
        const a0 = -Math.PI / 2 + ((i + gap) / STAGES.length) * Math.PI * 2;
        const a1 = -Math.PI / 2 + ((i + 1 - gap) / STAGES.length) * Math.PI * 2;
        const from = { x: CENTER_X + RADIUS * Math.cos(a0), y: CENTER + RADIUS * Math.sin(a0) };
        const to = { x: CENTER_X + RADIUS * Math.cos(a1), y: CENTER + RADIUS * Math.sin(a1) };
        return (
          <path
            key={`arc-${i}`}
            d={`M${from.x},${from.y} A${RADIUS},${RADIUS} 0 0 1 ${to.x},${to.y}`}
            fill="none"
            stroke="var(--axis)"
            strokeWidth={1.5}
            markerEnd="url(#ring-arrow)"
          />
        );
      })}
      {STAGES.map((name, i) => {
        const p = point(i);
        const label = point(i, RADIUS + 24);
        const active = name === stage;
        return (
          <g key={name}>
            <title>{active ? `${name} (current)` : name}</title>
            {active && live ? <circle className="ring-pulse" cx={p.x} cy={p.y} r={NODE} /> : null}
            <circle
              className={`ring-stage${active ? " active" : ""}`}
              cx={p.x}
              cy={p.y}
              r={NODE}
              fill="var(--surface)"
              stroke="var(--axis)"
              strokeWidth={2}
            />
            <text
              className={`ring-label${active ? " active" : ""}`}
              x={label.x}
              y={label.y + 4}
              textAnchor={label.x < CENTER_X - 10 ? "end" : label.x > CENTER_X + 10 ? "start" : "middle"}
            >
              {name}
            </text>
          </g>
        );
      })}
      <text x={CENTER_X} y={CENTER - 4} textAnchor="middle" style={{ fontSize: 12 }}>
        epoch
      </text>
      <text x={CENTER_X} y={CENTER + 20} textAnchor="middle" style={{ fontSize: 22, fill: "var(--ink)" }}>
        {epoch ?? "—"}
      </text>
    </svg>
  );
}
